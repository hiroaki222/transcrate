//! The desktop window's back end.
//!
//! Every decision — what a file is, what it would become, which players will
//! take it — belongs to `transcrate-core`. This crate starts the window, runs
//! ffmpeg on a worker thread and hands the answers over. Keeping it that thin
//! is what stops the window and the command line from drifting apart.

mod tools;
mod view;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use transcrate_core::device::{self, DeviceProfile};
use transcrate_core::files::{self, PreviousOutput};
use transcrate_core::plan::{Action, MetadataPolicy, Target};
use transcrate_core::{convert, usb};

use view::{DeviceRow, Drive, Lamp, Progress, Tools, Track};

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
            .ok_or_else(|| format!("{} という変換先はありません", self.profile))?;

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
            .map(|id| device::by_id(id).ok_or_else(|| format!("{id} という機材はありません")))
            .collect()
    }
}

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
    let inputs = gather(paths, PreviousOutput::Skip);

    let mut tracks = Vec::with_capacity(inputs.len());

    for (done, input) in inputs.iter().enumerate() {
        report(app, "inspect", done, inputs.len(), input);

        tracks.push(
            match convert::prepare(input, None, &tool(FFPROBE), &|_| target) {
                Ok(job) => describe(input, &job, &players),
                Err(error) => Track::unreadable(input, error.to_string()),
            },
        );
    }

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
    let inputs = gather(paths, PreviousOutput::Skip);

    let mut jobs = Vec::new();
    let mut failures = Vec::new();

    for input in &inputs {
        match convert::prepare(input, None, &tool(FFPROBE), &|_| target) {
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
    let finished = |index: usize, _outcome: &Result<(), convert::ConvertError>| {
        if let Some(job) = jobs.get(index) {
            report(app, "convert", index + 1, total, &job.input);
        }
    };

    let results = convert::run_all(
        &tool(FFMPEG),
        &jobs,
        convert::default_concurrency(),
        &finished,
    );

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
        filesystem: drive.filesystem.map(view::filesystem_name),
        reported_as: drive.reported_as,
        readable: lamps.iter().filter(|lamp| lamp.ok).count(),
        lamps,
    }))
}

fn gather(paths: &[String], previous: PreviousOutput) -> Vec<PathBuf> {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    files::collect(&paths, previous)
}

/// Tell the window how far along a sweep is.
///
/// A folder of a few hundred tracks would otherwise sit silent while ffprobe
/// works through it, which reads as a hang.
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
            convert_all,
            check_drive
        ])
        .run(tauri::generate_context!())
        .expect("could not start the window");
}
