//! Shared helpers for the integration tests.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Whether ffmpeg and ffprobe are both usable, skipping the caller if not.
///
/// A checkout without them still passes the rest of the suite. CI is held to a
/// stricter rule: these tests are the only thing checking that the argument
/// lists actually work, so a silent skip there would turn a green run into
/// nothing at all.
pub(crate) fn tools_available() -> bool {
    let present = !missing("ffmpeg") && !missing("ffprobe");

    assert!(
        present || std::env::var_os("CI").is_none(),
        "CI needs ffmpeg and ffprobe on PATH: these tests are the only ones that run \
         a real conversion, and skipping them would leave nothing checking it"
    );

    if !present {
        eprintln!("skipping: ffmpeg or ffprobe is not on PATH");
    }

    present
}

fn missing(tool: &str) -> bool {
    Command::new(tool).arg("-version").output().is_err()
}

/// A scratch directory of its own, so tests do not tread on each other.
pub(crate) fn workspace(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("transcrate-{name}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// Encode a fifth of a second of silence with the given codec settings.
pub(crate) fn encode(dir: &Path, name: &str, sample_rate_hz: u32, codec_args: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let source = format!("anullsrc=r={sample_rate_hz}:cl=stereo");

    let status = Command::new("ffmpeg")
        .args([
            "-v", "error", "-y", "-f", "lavfi", "-i", &source, "-t", "0.2",
        ])
        .args(codec_args)
        .arg(&path)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg failed to write {name}");

    path
}
