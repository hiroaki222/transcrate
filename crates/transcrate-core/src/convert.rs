//! Carrying out a plan.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::compat::AudioSpec;
use crate::files::{self, PathError};
use crate::parallel;
use crate::plan::{Action, Plan, Target, encode_args};
use crate::probe::{self, ProbeError};

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("could not run ffmpeg: {source}")]
    NotRunnable {
        #[source]
        source: std::io::Error,
    },
    #[error("ffmpeg could not write the output: {stderr}")]
    Failed { stderr: String },
    #[error("could not copy the file: {source}")]
    CopyFailed {
        #[source]
        source: std::io::Error,
    },
    #[error("could not create {directory}: {source}")]
    NoOutputFolder {
        directory: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Why a file could not be turned into a job, before any of it ran.
#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("{path}: {source}")]
    Unreadable {
        path: PathBuf,
        #[source]
        source: ProbeError,
    },
    #[error("{0}")]
    Destination(#[from] PathError),
    #[error("{path} and {other} would both be written to {output}")]
    SameDestination {
        path: PathBuf,
        other: PathBuf,
        output: PathBuf,
    },
    #[error("{path} would be written over {other}, which this run is reading")]
    OverSource { path: PathBuf, other: PathBuf },
}

/// Work out what would happen to `input`, without doing any of it.
///
/// Read-only on purpose. This is what a preview calls, and someone looking at a
/// list of what a conversion *would* produce has not agreed to anything landing
/// on their disk yet — not even an empty folder.
///
/// `target_for` is handed the source so a target can be built from the file
/// itself, which is what lets one run cover a folder of mixed formats.
///
/// # Errors
///
/// Fails when ffprobe cannot read the file, and when the result would have
/// nowhere to go.
pub fn prepare(
    input: &Path,
    into: Option<&Path>,
    base: Option<&Path>,
    ffprobe: &Path,
    target_for: &dyn Fn(&AudioSpec) -> Target,
) -> Result<Job, PrepareError> {
    let source = probe::run(ffprobe, input).map_err(|source| PrepareError::Unreadable {
        path: input.to_path_buf(),
        source,
    })?;

    let plan = crate::plan::plan(&source, &target_for(&source));
    let output = files::output_path(input, into, base, plan.output.codec)?;

    Ok(Job {
        plan,
        input: input.to_path_buf(),
        output,
    })
}

/// Carry out `plan`, writing to `output`.
///
/// A plan that asks for a copy gets one: running the bytes through an encoder
/// to produce the same format would cost time and, for a lossy source, quality.
///
/// # Errors
///
/// Fails when ffmpeg cannot be started, when it rejects the conversion, or when
/// a copy cannot be written.
pub fn run(ffmpeg: &Path, plan: &Plan, input: &Path, output: &Path) -> Result<(), ConvertError> {
    // Made here rather than when the job was worked out, so that previewing a
    // library leaves no trace of a conversion nobody asked for.
    if let Some(directory) = output.parent() {
        std::fs::create_dir_all(directory).map_err(|source| ConvertError::NoOutputFolder {
            directory: directory.to_path_buf(),
            source,
        })?;
    }

    match plan.action {
        Action::Copy => std::fs::copy(input, output)
            .map(|_| ())
            .map_err(|source| ConvertError::CopyFailed { source }),
        Action::Retag | Action::Encode { .. } => encode(ffmpeg, plan, input, output),
    }
}

/// One file's conversion.
#[derive(Debug, Clone)]
pub struct Job {
    pub plan: Plan,
    pub input: PathBuf,
    pub output: PathBuf,
}

/// Plan every file, at most `concurrency` at a time.
///
/// Planning is one ffprobe process per file, so a folder of several hundred
/// tracks spends real time here before a single note is encoded. Results stay in
/// the order the files were given, because a failure has to name its file.
///
/// # Panics
///
/// Panics if a worker thread panics.
pub fn prepare_all(
    found: &files::Found,
    into: Option<&Path>,
    ffprobe: &Path,
    target_for: &(dyn Fn(&AudioSpec) -> Target + Sync),
    concurrency: usize,
    on_finished: &(dyn Fn(usize, &Result<Job, PrepareError>) + Sync),
) -> Vec<Result<Job, PrepareError>> {
    let mut prepared = parallel::map(
        &found.files,
        concurrency,
        &|_, file| prepare(file, into, found.base_of(file), ffprobe, target_for),
        on_finished,
    );

    refuse_clashes(&mut prepared);
    prepared
}

/// What makes a job unsafe to run beside the others it was prepared with.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Clash {
    /// Another job in the batch writes to the same place.
    SameOutput(usize),
    /// The output would land on a file this batch is reading.
    OverInput(usize),
}

/// Find the jobs in a batch that would destroy each other's work.
///
/// A destination is built from the source's stem and the target's extension and
/// nothing else, so `mix.wav` and `mix.flac` sitting in one folder both ask for
/// `mix.mp3`. Nothing further down notices: ffmpeg is given `-y`, and a copy
/// overwrites as readily, so the two race and only whichever landed last
/// survives — with no error anywhere to say a conversion was lost.
///
/// Both sides of a collision are refused, not one. There is no way to tell from
/// here which file was meant, and quietly keeping one is the failure being
/// fixed.
///
/// `pairs` holds an input and an output for every job that was prepared, and
/// `None` where preparing already failed. The returned verdicts line up with it.
fn clashes(pairs: &[Option<(&Path, &Path)>]) -> Vec<Option<Clash>> {
    let mut verdicts = vec![None; pairs.len()];
    let mut writers: HashMap<OsString, usize> = HashMap::new();

    for (index, pair) in pairs.iter().enumerate() {
        let Some((_, output)) = pair else { continue };

        match writers.entry(same_file_key(output)) {
            Entry::Vacant(slot) => {
                slot.insert(index);
            }
            Entry::Occupied(slot) => {
                let first = *slot.get();
                verdicts[index] = Some(Clash::SameOutput(first));
                // The one already recorded named nobody, because when it was
                // seen it was still the only claim on that name.
                verdicts[first].get_or_insert(Clash::SameOutput(index));
            }
        }
    }

    let mut readers: HashMap<OsString, usize> = HashMap::new();
    for (index, pair) in pairs.iter().enumerate() {
        if let Some((input, _)) = pair {
            readers.entry(same_file_key(input)).or_insert(index);
        }
    }

    for (index, pair) in pairs.iter().enumerate() {
        let Some((_, output)) = pair else { continue };
        if verdicts[index].is_some() {
            continue;
        }

        // A job landing on its own source is caught when the destination is
        // worked out; this is the same file belonging to a *different* job.
        if let Some(&other) = readers.get(&same_file_key(output))
            && other != index
        {
            verdicts[index] = Some(Clash::OverInput(other));
        }
    }

    verdicts
}

/// A path as the filesystem would compare it.
///
/// APFS, exFAT and NTFS all treat two names differing only in case as one file,
/// so `Mix.mp3` and `mix.mp3` are a collision there. On a case-sensitive volume
/// the same comparison only ever refuses a pair that would in fact have worked,
/// which is the safe way to be wrong about it.
fn same_file_key(path: &Path) -> OsString {
    // Anchored, then flattened. One run can be handed both spellings of a
    // file — `convert /Users/me/Music/a.wav a.flac` from inside that folder —
    // and compared as written they are two keys and the clash goes unseen.
    let anchored = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };

    let settled = files::without_dot_segments(&anchored);

    if cfg!(any(target_os = "macos", target_os = "windows")) {
        OsString::from(settled.to_string_lossy().to_lowercase())
    } else {
        settled.into_os_string()
    }
}

fn refuse_clashes(prepared: &mut [Result<Job, PrepareError>]) {
    let pairs: Vec<Option<(PathBuf, PathBuf)>> = prepared
        .iter()
        .map(|outcome| {
            outcome
                .as_ref()
                .ok()
                .map(|job| (job.input.clone(), job.output.clone()))
        })
        .collect();

    let verdicts = {
        let borrowed: Vec<Option<(&Path, &Path)>> = pairs
            .iter()
            .map(|pair| {
                pair.as_ref()
                    .map(|(input, output)| (input.as_path(), output.as_path()))
            })
            .collect();

        clashes(&borrowed)
    };

    for (index, verdict) in verdicts.into_iter().enumerate() {
        let Some(verdict) = verdict else { continue };
        let Some((path, output)) = pairs[index].clone() else {
            continue;
        };

        let named = match verdict {
            Clash::SameOutput(other) | Clash::OverInput(other) => other,
        };
        let Some((other, _)) = pairs[named].clone() else {
            continue;
        };

        prepared[index] = Err(match verdict {
            Clash::SameOutput(_) => PrepareError::SameDestination {
                path,
                other,
                output,
            },
            Clash::OverInput(_) => PrepareError::OverSource { path, other },
        });
    }
}

/// Run every job, at most `concurrency` at a time.
///
/// Results come back in the order the jobs were given, whatever order they
/// finished in. Out of order they would be useless: a failure has to name the
/// file that caused it.
///
/// `on_finished` is called from a worker thread as each job lands, with the
/// job's index. A folder of a hundred tracks would otherwise sit silent until
/// the last one finished.
///
/// # Panics
///
/// Panics if a worker thread panics, which poisons the shared results and
/// leaves the run with no answer for at least one job.
pub fn run_all(
    ffmpeg: &Path,
    jobs: &[Job],
    concurrency: usize,
    on_finished: &(dyn Fn(usize, &Result<(), ConvertError>) + Sync),
) -> Vec<Result<(), ConvertError>> {
    parallel::map(
        jobs,
        concurrency,
        &|_, job| run(ffmpeg, &job.plan, &job.input, &job.output),
        on_finished,
    )
}

fn encode(ffmpeg: &Path, plan: &Plan, input: &Path, output: &Path) -> Result<(), ConvertError> {
    let result = Command::new(ffmpeg)
        // -nostdin matters once these run in parallel: without it every ffmpeg
        // reaches for the terminal and they fight over it.
        .args(["-nostdin", "-v", "error", "-y", "-i"])
        .arg(input)
        .args(encode_args(plan))
        .arg(output)
        .output()
        .map_err(|source| ConvertError::NotRunnable { source })?;

    if result.status.success() {
        return Ok(());
    }

    let said = String::from_utf8_lossy(&result.stderr).trim().to_owned();
    Err(ConvertError::Failed {
        // Ending the message at the colon says only that something went wrong.
        // The status separates a file ffmpeg refused from a path that is not
        // ffmpeg at all.
        stderr: if said.is_empty() {
            format!("it wrote nothing and exited with {}", result.status)
        } else {
            said
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A prepared slot. The `Some` is the point: these sit in a table beside
    /// `None`s standing for files that never got as far as a destination.
    #[allow(clippy::unnecessary_wraps)]
    fn pair<'a>(input: &'a str, output: &'a str) -> Option<(&'a Path, &'a Path)> {
        Some((Path::new(input), Path::new(output)))
    }

    /// The everyday version of this: one track kept in two formats, converted
    /// in the same run. Both asked for `_transcrate/mix.mp3`, both were run,
    /// and the slower one overwrote the faster one's work.
    #[test]
    fn two_sources_writing_one_file_are_both_refused() {
        let verdicts = clashes(&[
            pair("/music/mix.wav", "/music/_transcrate/mix.mp3"),
            pair("/music/mix.flac", "/music/_transcrate/mix.mp3"),
            pair("/music/other.wav", "/music/_transcrate/other.mp3"),
        ]);

        assert_eq!(verdicts[0], Some(Clash::SameOutput(1)));
        assert_eq!(verdicts[1], Some(Clash::SameOutput(0)));
        assert_eq!(verdicts[2], None, "a name nobody else claims is fine");
    }

    /// Reached with `--into` pointed at a folder the sources are already in.
    /// The loss here is a source file, not a conversion.
    #[test]
    fn an_output_landing_on_another_source_is_refused() {
        let verdicts = clashes(&[
            pair("/music/mix.wav", "/music/mix.mp3"),
            pair("/music/mix.mp3", "/elsewhere/mix.mp3"),
        ]);

        assert_eq!(verdicts[0], Some(Clash::OverInput(1)));
        assert_eq!(verdicts[1], None);
    }

    /// Files that failed to prepare hold a slot so that every verdict lines up
    /// with the job it belongs to.
    #[test]
    fn slots_that_never_prepared_are_skipped_without_shifting_the_rest() {
        let verdicts = clashes(&[
            None,
            pair("/music/mix.wav", "/music/out/mix.mp3"),
            None,
            pair("/music/mix.aiff", "/music/out/mix.mp3"),
        ]);

        assert_eq!(verdicts.len(), 4);
        assert_eq!(verdicts[0], None);
        assert_eq!(verdicts[1], Some(Clash::SameOutput(3)));
        assert_eq!(verdicts[3], Some(Clash::SameOutput(1)));
    }

    /// One destination spelled two ways is one destination. Compared as written,
    /// the clash is invisible and both conversions run.
    #[test]
    fn a_destination_spelled_another_way_is_the_same_destination() {
        let verdicts = clashes(&[
            pair("/music/mix.wav", "/music/out/mix.mp3"),
            pair("/music/mix.flac", "/music/sets/../out/./mix.mp3"),
        ]);

        assert_eq!(verdicts[0], Some(Clash::SameOutput(1)));
        assert_eq!(verdicts[1], Some(Clash::SameOutput(0)));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn names_differing_only_in_case_are_one_file_here() {
        let verdicts = clashes(&[
            pair("/music/Mix.wav", "/music/out/Mix.mp3"),
            pair("/music/mix.flac", "/music/out/mix.mp3"),
        ]);

        assert_eq!(verdicts[0], Some(Clash::SameOutput(1)));
    }
}
