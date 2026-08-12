//! Carrying out a plan.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::compat::AudioSpec;
use crate::files::{self, PathError};
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
    ffprobe: &Path,
    target_for: &dyn Fn(&AudioSpec) -> Target,
) -> Result<Job, PrepareError> {
    let source = probe::run(ffprobe, input).map_err(|source| PrepareError::Unreadable {
        path: input.to_path_buf(),
        source,
    })?;

    let plan = crate::plan::plan(&source, &target_for(&source));
    let output = files::output_path(input, into, plan.output.codec)?;

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

/// How many encodes to run at once by default: one per available core.
///
/// Each ffmpeg is pinned to a single thread, because audio codecs barely
/// parallelise — throughput comes from running many encodes at once rather than
/// from making one of them faster.
pub fn default_concurrency() -> usize {
    thread::available_parallelism().map_or(1, NonZeroUsize::get)
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
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<(), ConvertError>>>> =
        Mutex::new((0..jobs.len()).map(|_| None).collect());

    // Workers beyond the job count would start only to find nothing left.
    let workers = concurrency.clamp(1, jobs.len().max(1));

    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(job) = jobs.get(index) else { break };

                    let outcome = run(ffmpeg, &job.plan, &job.input, &job.output);
                    on_finished(index, &outcome);

                    // Held just long enough to drop the result into its slot,
                    // never across an encode.
                    results.lock().expect("results lock")[index] = Some(outcome);
                }
            });
        }
    });

    results
        .into_inner()
        .expect("results lock")
        .into_iter()
        .map(|slot| slot.expect("every index was filled by a worker"))
        .collect()
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

    Err(ConvertError::Failed {
        stderr: String::from_utf8_lossy(&result.stderr).trim().to_owned(),
    })
}
