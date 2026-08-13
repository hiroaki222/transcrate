//! The desktop window's back end.
//!
//! Every decision — what a file is, what it would become, which players will
//! take it — belongs to `transcrate-core`. This crate starts the window, runs
//! ffmpeg on a worker thread and hands the answers over. Keeping it that thin
//! is what stops the window and the command line from drifting apart.

mod tools;
mod view;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use transcrate_core::device::{self, DeviceProfile};
use transcrate_core::files::{self, PreviousOutput};
use transcrate_core::plan::{self, Action, MetadataPolicy, Target};
use transcrate_core::{convert, parallel, scan, usb};

use view::{Contents, DeviceRow, Drive, Lamp, Mounted, Progress, Tools, Track};

/// What the window has chosen, as it stands when a command is issued.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Settings {
    /// A profile name (`cdj-safe`) or a bare format name (`aiff`).
    pub(crate) profile: String,
    pub(crate) keep_comment: bool,
    pub(crate) artwork: bool,
    /// Players to judge against. Empty means every one in the table.
    pub(crate) devices: Vec<String>,
}

impl Settings {
    fn target(&self) -> Result<Target, String> {
        let base = Target::by_name(&self.profile)
            .or_else(|| Target::from_format(&self.profile))
            .ok_or_else(|| format!("no target named {}", self.profile))?;

        let metadata = if self.keep_comment {
            MetadataPolicy::DJ
        } else {
            MetadataPolicy::CLEARING_COMMENTS
        };

        Ok(Target {
            metadata: if self.artwork {
                metadata
            } else {
                metadata.without_artwork()
            },
            ..base
        })
    }

    fn players(&self) -> Result<Vec<&'static DeviceProfile>, String> {
        if self.devices.is_empty() {
            return Ok(device::DEVICES.iter().collect());
        }

        self.devices
            .iter()
            .map(|id| device::by_id(id).ok_or_else(|| format!("no player named {id}")))
            .collect()
    }
}

/// Said of a folder the sweep could not list. Whatever audio was inside it was
/// never planned, so without this the list simply comes back shorter.
const UNREADABLE_FOLDER: &str = "could not be read, and nothing inside it was checked";

/// One file's outcome, kept alongside the index it was given at.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Outcome {
    pub(crate) path: String,
    pub(crate) name: String,
    pub(crate) output_path: String,
    pub(crate) error: Option<String>,
}

const FFMPEG: &str = "ffmpeg";
const FFPROBE: &str = "ffprobe";

/// Where a tool is, preferring the copy shipped beside the app.
fn tool(name: &str) -> PathBuf {
    tools::locate(tools::alongside().as_deref(), name)
}

/// Whether the two binaries every conversion needs can be run.
#[tauri::command]
fn tools() -> Tools {
    Tools {
        ffmpeg: tools::runnable(&tool(FFMPEG)),
        ffprobe: tools::runnable(&tool(FFPROBE)),
    }
}

/// The language the machine is set to, as a BCP 47 tag such as `ja-JP`.
///
/// Read from the operating system rather than from the webview: an app that
/// ships no localisation of its own gets told English there whatever the user
/// actually set.
#[tauri::command]
fn locale() -> Option<String> {
    sys_locale::get_locale()
}

/// The compatibility table, in the order it is always shown in.
#[tauri::command]
fn devices() -> Vec<DeviceRow> {
    view::device_rows(device::DEVICES)
}

/// Work out what would happen to every file behind `paths`, without doing it.
///
/// Read-only: nothing is written, not even the folder the results would go in.
/// Somebody who has dropped a library onto the window has not agreed to
/// anything landing on their disk.
// Tauri deserialises command arguments into owned values before the call, so
// these cannot take references however little they consume.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
async fn inspect(
    app: AppHandle,
    paths: Vec<String>,
    settings: Settings,
) -> Result<Vec<Track>, String> {
    tauri::async_runtime::spawn_blocking(move || examine(&app, &paths, &settings))
        .await
        .map_err(|error| error.to_string())?
}

fn examine(app: &AppHandle, paths: &[String], settings: &Settings) -> Result<Vec<Track>, String> {
    let target = settings.target()?;
    let players = settings.players()?;
    let found = gather(paths, PreviousOutput::Skip);
    let inputs = &found.files;

    let done = Mutex::new(0usize);
    let prepared = convert::prepare_all(
        &found,
        None,
        &tool(FFPROBE),
        &|_| target,
        parallel::default_concurrency(),
        &|index, _| advance(app, "inspect", &done, inputs.len(), &inputs[index]),
    );

    // A folder that would not open holds tracks nobody will see otherwise.
    // Listed as itself, it says so where the tracks it hid would have been.
    let tracks = found
        .unreadable
        .iter()
        .map(|folder| Track::unreadable(folder, UNREADABLE_FOLDER.to_owned()))
        .chain(
            inputs
                .iter()
                .zip(prepared)
                .map(|(input, outcome)| match outcome {
                    Ok(job) => describe(input, &job, &players),
                    Err(error) => Track::unreadable(input, error.to_string()),
                }),
        )
        .collect();

    report(app, "inspect", inputs.len(), inputs.len(), Path::new(""));
    Ok(tracks)
}

fn describe(input: &Path, job: &convert::Job, players: &[&'static DeviceProfile]) -> Track {
    Track {
        path: input.display().to_string(),
        name: view::file_name(input),
        source: Some(job.plan.source),
        output: Some(job.plan.output),
        output_path: Some(job.output.display().to_string()),
        action: Some(view::action_name(job.plan.action)),
        dither: matches!(job.plan.action, Action::Encode { dither: true }),
        thin: plan::sounds_thin(&job.plan.source),
        now: view::lamps_for(&job.plan.source, players),
        after: view::lamps_for(&job.plan.output, players),
        error: None,
    }
}

/// Convert every file behind `paths`, one job per core.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
async fn convert_all(
    app: AppHandle,
    paths: Vec<String>,
    settings: Settings,
) -> Result<Vec<Outcome>, String> {
    tauri::async_runtime::spawn_blocking(move || encode(&app, &paths, &settings))
        .await
        .map_err(|error| error.to_string())?
}

fn encode(app: &AppHandle, paths: &[String], settings: &Settings) -> Result<Vec<Outcome>, String> {
    let target = settings.target()?;
    let found = gather(paths, PreviousOutput::Skip);
    let inputs = &found.files;

    let concurrency = parallel::default_concurrency();
    let prepared = convert::prepare_all(
        &found,
        None,
        &tool(FFPROBE),
        &|_| target,
        concurrency,
        &|_, _| {},
    );

    let mut jobs = Vec::new();
    let mut failures: Vec<Outcome> = found
        .unreadable
        .iter()
        .map(|folder| Outcome {
            path: folder.display().to_string(),
            name: view::file_name(folder),
            output_path: String::new(),
            error: Some(UNREADABLE_FOLDER.to_owned()),
        })
        .collect();

    for (input, outcome) in inputs.iter().zip(prepared) {
        match outcome {
            Ok(job) => jobs.push(job),
            Err(error) => failures.push(Outcome {
                path: input.display().to_string(),
                name: view::file_name(input),
                output_path: String::new(),
                error: Some(error.to_string()),
            }),
        }
    }

    let total = jobs.len();
    let done = Mutex::new(0usize);
    let finished = |index: usize, _outcome: &Result<(), convert::ConvertError>| {
        if let Some(job) = jobs.get(index) {
            // How many have landed, not where this one sat in the list. With
            // several encoders running, a short track further down finishes
            // first, and its position would send the count forward and then
            // back again.
            advance(app, "convert", &done, total, &job.input);
        }
    };

    let results = convert::run_all(&tool(FFMPEG), &jobs, concurrency, &finished);

    let mut outcomes: Vec<_> = jobs
        .iter()
        .zip(results)
        .map(|(job, result)| Outcome {
            path: job.input.display().to_string(),
            name: view::file_name(&job.input),
            output_path: job.output.display().to_string(),
            error: result.err().map(|error| error.to_string()),
        })
        .collect();

    outcomes.append(&mut failures);
    Ok(outcomes)
}

/// Every drive that could be carried to a gig, for the picker.
///
/// Cheap: no file on any of them is opened. The window asks again each time the
/// screen is opened, because a stick plugged in after the app started is the
/// ordinary case rather than the exception.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn drives(settings: Settings) -> Result<Vec<Mounted>, String> {
    let players = settings.players()?;

    Ok(usb::drives()
        .into_iter()
        .map(|drive| Mounted {
            mount_point: drive.mount_point.display().to_string(),
            name: drive.name,
            filesystem: drive.filesystem.map(view::filesystem_name),
            reported_as: drive.reported_as,
            readable: drive.filesystem.map_or(0, |filesystem| {
                players
                    .iter()
                    .filter(|player| player.filesystem_support(filesystem) == device::Support::Yes)
                    .count()
            }),
            players: players.len(),
            total_bytes: drive.total_bytes,
            free_bytes: drive.free_bytes,
        })
        .collect())
}

/// Which players will read the drive holding `path`.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
fn check_drive(path: String, settings: Settings) -> Result<Option<Drive>, String> {
    let players = settings.players()?;

    let Some(drive) = usb::drive_at(Path::new(&path)) else {
        return Ok(None);
    };

    // A filesystem this program does not recognise is one no player reads, so
    // every lamp goes out rather than being left blank.
    let lamps: Vec<Lamp> = players
        .iter()
        .map(|player| Lamp {
            id: player.id,
            name: player.display_name,
            short: player.lamp_name,
            ok: drive.filesystem.is_some_and(|filesystem| {
                player.filesystem_support(filesystem) == device::Support::Yes
            }),
            issues: Vec::new(),
        })
        .collect();

    Ok(Some(Drive {
        mount_point: drive.mount_point.display().to_string(),
        name: drive.name,
        filesystem: drive.filesystem.map(view::filesystem_name),
        reported_as: drive.reported_as,
        readable: lamps.iter().filter(|lamp| lamp.ok).count(),
        lamps,
    }))
}

/// What is on the drive, and which of it the players will take.
///
/// Separate from `check_drive` because it is the slow half: the walk is
/// immediate, but reading the tracks is one ffprobe each and a full stick is
/// thousands of them. The window shows the filesystem verdict straight away and
/// fills this in as it arrives.
#[allow(clippy::needless_pass_by_value)]
#[tauri::command]
async fn scan_drive(
    app: AppHandle,
    path: String,
    settings: Settings,
) -> Result<Option<Contents>, String> {
    tauri::async_runtime::spawn_blocking(move || sweep(&app, &path, &settings))
        .await
        .map_err(|error| error.to_string())?
}

fn sweep(app: &AppHandle, path: &str, settings: &Settings) -> Result<Option<Contents>, String> {
    let players = settings.players()?;
    let Some(limits) = scan::Limits::strictest_of(&players) else {
        return Ok(None);
    };

    // The drive, not the folder that was pointed at: a limit is a property of
    // the stick, and half of it measured is a wrong answer rather than a partial one.
    let root = match usb::drive_at(Path::new(path)) {
        Some(drive) => drive.mount_point,
        None => PathBuf::from(path),
    };

    let contents = scan::walk(&root, limits);
    let total = contents.tracks.len();

    let done = Mutex::new(0usize);
    let verdicts = scan::judge(
        &contents.tracks,
        &tool(FFPROBE),
        &players,
        parallel::default_concurrency(),
        &|index| advance(app, "scan", &done, total, &contents.tracks[index]),
    );

    report(app, "scan", total, total, Path::new(""));

    let relative = |path: &Path| {
        path.strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string()
    };

    Ok(Some(Contents {
        tracks: total,
        folders: contents.folders,
        other_files: contents.other_files,
        deepest: contents.deepest,
        depth_limit: limits.folder_depth,
        entry_limit: limits.entries_per_folder,
        unreachable: contents.unreachable.iter().map(|f| relative(f)).collect(),
        unreadable: contents.unreadable.iter().map(|f| relative(f)).collect(),
        crowded: contents
            .crowded
            .iter()
            .map(|crowded| view::Crowded {
                folder: relative(&crowded.folder),
                entries: crowded.entries,
            })
            .collect(),
        failing: verdicts
            .iter()
            .filter(|verdict| !verdict.plays())
            .map(|verdict| view::FailingTrack {
                path: verdict.path.display().to_string(),
                name: view::file_name(&verdict.path),
                folder: verdict.path.parent().map_or_else(String::new, &relative),
                spec: verdict.spec,
                lamps: verdict
                    .spec
                    .as_ref()
                    .map_or_else(Vec::new, |spec| view::lamps_for(spec, &players)),
                error: verdict.error.clone(),
            })
            .collect(),
    }))
}

fn gather(paths: &[String], previous: PreviousOutput) -> files::Found {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    files::collect(&paths, previous)
}

/// Tell the window how far along a sweep is.
///
/// A folder of a few hundred tracks would otherwise sit silent while ffprobe
/// works through it, which reads as a hang.
/// One more finished, told to the window as it lands.
///
/// The count and the event go out under one lock. Counted with an atomic and
/// emitted afterwards, two workers can take their numbers in order and reach
/// the window in the other one, and the meter runs backwards.
fn advance(app: &AppHandle, stage: &str, done: &Mutex<usize>, total: usize, current: &Path) {
    let mut count = done.lock().unwrap_or_else(PoisonError::into_inner);
    *count += 1;
    report(app, stage, *count, total, current);
}

fn report(app: &AppHandle, stage: &str, done: usize, total: usize, current: &Path) {
    let _ = app.emit(
        stage,
        Progress {
            done,
            total,
            name: view::file_name(current),
        },
    );
}

/// # Panics
///
/// Panics if the window cannot be created, which leaves nothing to show.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            tools,
            locale,
            devices,
            inspect,
            drives,
            scan_drive,
            convert_all,
            check_drive
        ])
        .run(tauri::generate_context!())
        .expect("could not start the window");
}
