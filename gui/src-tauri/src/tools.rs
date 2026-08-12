//! Finding ffmpeg.
//!
//! A released build carries its own copy beside the executable, because the
//! people this window exists for are the ones who will not install one. A
//! checkout has no such copy and falls back to the PATH, which is also what
//! anyone who keeps their own build would want.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The name a tool is installed under on this platform.
pub(crate) fn file_name(tool: &str) -> String {
    if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_owned()
    }
}

/// Prefer the copy shipped beside the app, then whatever is on the PATH.
///
/// `beside` is the directory holding the running executable, which is where
/// Tauri puts a sidecar on every platform it bundles for.
pub(crate) fn locate(beside: Option<&Path>, tool: &str) -> PathBuf {
    let shipped = beside.map(|directory| directory.join(file_name(tool)));

    match shipped {
        Some(path) if path.is_file() => path,
        // A bare name is resolved against the PATH by the operating system,
        // which is the behaviour a checkout wants.
        _ => PathBuf::from(tool),
    }
}

/// The directory the running executable sits in, if it can be determined.
pub(crate) fn alongside() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()?
        .parent()
        .map(Path::to_path_buf)
}

/// Whether a tool can actually be run.
pub(crate) fn runnable(tool: &Path) -> bool {
    Command::new(tool).arg("-version").output().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A released build must not reach past its own copy to a system install,
    /// which could be any version and any set of encoders.
    #[test]
    fn a_shipped_copy_wins() {
        let directory = std::env::temp_dir().join("transcrate-tools-shipped");
        std::fs::create_dir_all(&directory).expect("create");

        let shipped = directory.join(file_name("ffmpeg"));
        std::fs::write(&shipped, b"").expect("write");

        assert_eq!(locate(Some(&directory), "ffmpeg"), shipped);
    }

    /// A checkout has no sidecar, and falling back to a bare name lets the
    /// operating system search the PATH.
    #[test]
    fn without_one_the_path_is_used() {
        let empty = std::env::temp_dir().join("transcrate-tools-empty");
        let _ = std::fs::remove_dir_all(&empty);
        std::fs::create_dir_all(&empty).expect("create");

        assert_eq!(locate(Some(&empty), "ffmpeg"), Path::new("ffmpeg"));
        assert_eq!(locate(None, "ffmpeg"), Path::new("ffmpeg"));
    }

    /// A directory named `ffmpeg` beside the app is not an ffmpeg.
    #[test]
    fn a_directory_is_not_a_tool() {
        let directory = std::env::temp_dir().join("transcrate-tools-dir");
        std::fs::create_dir_all(directory.join(file_name("ffmpeg"))).expect("create");

        assert_eq!(locate(Some(&directory), "ffmpeg"), Path::new("ffmpeg"));
    }

    #[cfg(windows)]
    #[test]
    fn windows_tools_carry_an_extension() {
        assert_eq!(file_name("ffmpeg"), "ffmpeg.exe");
    }

    #[cfg(not(windows))]
    #[test]
    fn elsewhere_the_name_is_the_name() {
        assert_eq!(file_name("ffmpeg"), "ffmpeg");
    }
}
