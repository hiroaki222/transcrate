//! Carrying out a plan.

use std::path::Path;
use std::process::Command;

use crate::plan::{Action, Plan, encode_args};

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
    match plan.action {
        Action::Copy => std::fs::copy(input, output)
            .map(|_| ())
            .map_err(|source| ConvertError::CopyFailed { source }),
        Action::Encode { .. } => encode(ffmpeg, plan, input, output),
    }
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
