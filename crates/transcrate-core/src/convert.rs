//! Carrying out a plan.

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
    files: &[PathBuf],
    into: Option<&Path>,
    ffprobe: &Path,
    target_for: &(dyn Fn(&AudioSpec) -> Target + Sync),
    concurrency: usize,
    on_finished: &(dyn Fn(usize, &Result<Job, PrepareError>) + Sync),
) -> Vec<Result<Job, PrepareError>> {
    parallel::map(
        files,
        concurrency,
        &|_, file| prepare(file, into, ffprobe, target_for),
        on_finished,
    )
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

    Err(ConvertError::Failed {
        stderr: String::from_utf8_lossy(&result.stderr).trim().to_owned(),
    })
}
