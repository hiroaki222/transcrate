//! Turning what someone hands over into a list of tracks.
//!
//! A path typed at a shell, expanded by a glob or dropped onto a window all
//! arrive here as the same thing, and all three want the same answers: which of
//! these are audio, what is inside that folder, and where does the result go.

use std::path::{Path, PathBuf};

use crate::device::Codec;

/// The containers this program reads.
///
/// One list, used to sweep a folder, to build the shell completion and to
/// filter a drop — so the three cannot drift apart.
pub const AUDIO_EXTENSIONS: [&str; 8] = ["wav", "flac", "aif", "aiff", "m4a", "mp3", "aac", "mp4"];

/// Where converted files go.
pub const OUTPUT_FOLDER: &str = "_transcrate";

/// Whether a sweep descends into a previous run's output folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviousOutput {
    /// Converting: taking it back in would re-encode the last run's results,
    /// and for a lossy format that means losing a little more each time.
    Skip,
    /// Checking: "did what I made come out playable" is the obvious question to
    /// ask of a folder of conversions, so it has to be answerable.
    Include,
}

/// Nothing was wrong with the file — only with where its result would land.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PathError {
    #[error("{0} has no file name")]
    Nameless(PathBuf),
    #[error("refusing to overwrite the source: {0}")]
    WouldOverwriteSource(PathBuf),
}

/// Expand directories into the audio inside them, recursively.
///
/// One path on its own is taken at its word, whatever it is called: someone who
/// typed a single filename meant that file, and ffprobe judges it better than
/// the extension does. Several at once came from a glob or a drop, and both
/// hand over the artwork and the playlists too — so there, only audio comes
/// through.
pub fn collect(paths: &[PathBuf], previous: PreviousOutput) -> Vec<PathBuf> {
    let handed_over_in_bulk = paths.len() > 1;
    let mut found = Vec::new();

    for path in paths {
        if path.is_dir() {
            sweep(path, previous, &mut found);
        } else if !handed_over_in_bulk || is_audio(path) {
            found.push(path.clone());
        }
    }

    // read_dir returns entries in whatever order the filesystem holds them,
    // which would make the report jump around between runs.
    found.sort();
    found
}

fn sweep(directory: &Path, previous: PreviousOutput, into: &mut Vec<PathBuf>) {
    let is_previous_output = directory
        .file_name()
        .is_some_and(|name| name == OUTPUT_FOLDER);

    if is_previous_output && previous == PreviousOutput::Skip {
        return;
    }

    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sweep(&path, previous, into);
        } else if is_audio(&path) {
            into.push(path);
        }
    }
}

/// Whether a path names one of the containers this program reads.
pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
}

/// Where a converted file lands.
///
/// Defaults to a [`OUTPUT_FOLDER`] beside the input, so results sit next to the
/// tracks they came from and never inside the source library itself.
///
/// # Errors
///
/// Fails when the input has no file name, and when the result would land on top
/// of the source.
pub fn output_path(input: &Path, into: Option<&Path>, codec: Codec) -> Result<PathBuf, PathError> {
    let stem = input
        .file_stem()
        .ok_or_else(|| PathError::Nameless(input.to_path_buf()))?;

    let directory = match into {
        Some(directory) => directory.to_path_buf(),
        None => input.parent().unwrap_or(Path::new(".")).join(OUTPUT_FOLDER),
    };

    let mut destination = directory.join(stem);
    destination.set_extension(extension_for(codec));

    if destination == input {
        return Err(PathError::WouldOverwriteSource(input.to_path_buf()));
    }

    Ok(destination)
}

/// The extension a codec is normally written with.
pub const fn extension_for(codec: Codec) -> &'static str {
    match codec {
        Codec::Mp3 => "mp3",
        // The same ambiguity the reader has to cope with: both live in .m4a.
        Codec::AacLc | Codec::Alac => "m4a",
        Codec::Flac => "flac",
        Codec::PcmWav => "wav",
        Codec::PcmAiff => "aiff",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Codec;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("transcrate-files-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// Pointing at a folder is the common case — nobody wants to name four
    /// hundred tracks, and neither a shell glob nor a drop onto a window
    /// reaches into subfolders on its own.
    #[test]
    fn a_directory_expands_to_the_audio_inside_it() {
        let dir = scratch("collect");
        std::fs::create_dir_all(dir.join("sub")).expect("subdir");
        std::fs::write(dir.join("a.wav"), b"").expect("write");
        std::fs::write(dir.join("sub/b.flac"), b"").expect("write");
        std::fs::write(dir.join("cover.jpg"), b"").expect("write");
        std::fs::write(dir.join("notes.txt"), b"").expect("write");

        let names: Vec<_> = collect(&[dir], PreviousOutput::Skip)
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["a.wav", "b.flac"]);
    }

    /// Converting a folder twice must not convert the first run's output: that
    /// would re-encode a lossy file into itself, losing a little more each time.
    /// Checking one should look at it, though — "did what I made come out
    /// playable" is the obvious question to ask of a folder of conversions.
    #[test]
    fn only_converting_skips_a_previous_runs_output() {
        let dir = scratch("collect-previous-output");
        std::fs::create_dir_all(dir.join(OUTPUT_FOLDER)).expect("subdir");
        std::fs::write(dir.join("track.wav"), b"").expect("write");
        std::fs::write(dir.join(OUTPUT_FOLDER).join("track.mp3"), b"").expect("write");

        let names = |previous| {
            collect(std::slice::from_ref(&dir), previous)
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };

        assert_eq!(names(PreviousOutput::Skip), ["track.wav"]);
        assert_eq!(names(PreviousOutput::Include), ["track.mp3", "track.wav"]);
    }

    /// One path named on its own is taken at its word, whatever it is called.
    /// Someone who typed a single filename meant that file, and ffprobe is a
    /// better judge of it than the extension.
    #[test]
    fn a_single_named_file_is_taken_as_given() {
        let dir = scratch("collect-explicit");
        let odd = dir.join("no-extension");
        std::fs::write(&odd, b"").expect("write");

        assert_eq!(
            collect(std::slice::from_ref(&odd), PreviousOutput::Skip),
            vec![odd]
        );
    }

    /// `transcrate convert *` is the obvious way to do a folder from the shell,
    /// and the shell hands over everything in it: artwork, playlists, notes.
    /// Reporting each of those as a failure would bury the conversions.
    #[test]
    fn several_paths_at_once_keep_only_the_audio() {
        let dir = scratch("collect-glob");
        let expanded: Vec<_> = ["a.wav", "b.flac", "cover.jpg", "playlist.m3u", "notes.txt"]
            .iter()
            .map(|name| {
                let path = dir.join(name);
                std::fs::write(&path, b"").expect("write");
                path
            })
            .collect();

        let names: Vec<_> = collect(&expanded, PreviousOutput::Skip)
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["a.wav", "b.flac"]);
    }

    /// A `.WAV` ripped years ago is still a WAV.
    #[test]
    fn an_extension_is_recognised_whatever_its_case() {
        assert!(is_audio(Path::new("TRACK.WAV")));
        assert!(is_audio(Path::new("track.Flac")));
        assert!(!is_audio(Path::new("cover.jpg")));
        assert!(!is_audio(Path::new("no-extension")));
    }

    /// Results sit beside the tracks they came from, so a library is never
    /// written into.
    #[test]
    fn output_lands_in_a_folder_beside_the_input() {
        let output = output_path(Path::new("/music/track.flac"), None, Codec::Mp3).expect("path");

        assert_eq!(output, Path::new("/music/_transcrate/track.mp3"));
    }

    /// A named folder is used as given: this is what a GUI's "save into" is.
    #[test]
    fn a_named_folder_is_used_as_given() {
        let output = output_path(
            Path::new("/music/track.flac"),
            Some(Path::new("/stick/DJ")),
            Codec::PcmAiff,
        )
        .expect("path");

        assert_eq!(output, Path::new("/stick/DJ/track.aiff"));
    }

    /// Converting an MP3 into an MP3 in the folder it already sits in would
    /// destroy it, and the source is the one copy nobody has a backup of.
    #[test]
    fn writing_over_the_source_is_refused() {
        let refused = output_path(
            Path::new("/music/track.mp3"),
            Some(Path::new("/music")),
            Codec::Mp3,
        );

        assert!(matches!(refused, Err(PathError::WouldOverwriteSource(_))));
    }

    /// Both live in `.m4a`, which is the whole reason a codec is read from the
    /// stream rather than the extension.
    #[test]
    fn aac_and_alac_share_an_extension() {
        assert_eq!(extension_for(Codec::AacLc), "m4a");
        assert_eq!(extension_for(Codec::Alac), "m4a");
    }
}
