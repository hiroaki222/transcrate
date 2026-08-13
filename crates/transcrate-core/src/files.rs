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
///
/// `.mp4` is not here. A music video carries an AAC stream like a track does,
/// and reading only the audio would judge one as a track and report it as
/// playing everywhere. A folder of sets with a video in it is an ordinary
/// thing to have.
pub const AUDIO_EXTENSIONS: [&str; 7] = ["wav", "flac", "aif", "aiff", "m4a", "mp3", "aac"];

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
pub fn collect(paths: &[PathBuf], previous: PreviousOutput) -> Found {
    let handed_over_in_bulk = paths.len() > 1;
    let mut found = Found::default();

    for path in paths {
        if path.is_dir() {
            found.roots.push(path.clone());
            sweep(path, previous, &mut found);
        } else if !handed_over_in_bulk || is_audio(path) {
            found.files.push(path.clone());
        }
    }

    // read_dir returns entries in whatever order the filesystem holds them,
    // which would make the report jump around between runs.
    found.files.sort();
    found.unreadable.sort();
    found
}

/// What a set of paths expanded to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Every audio file, in a stable order.
    pub files: Vec<PathBuf>,
    /// Folders that could not be listed in full: permission refused, a drive
    /// pulled out part way through, a directory the filesystem would not read.
    /// Whatever audio is inside them is missing from `files`, so a caller that
    /// drops this reports a partial run as a complete one.
    pub unreadable: Vec<PathBuf>,
    /// The folders that were handed over and expanded, in the order given.
    pub roots: Vec<PathBuf>,
}

impl Found {
    /// The folder `file` was swept out of, if it was swept out of one.
    ///
    /// A file named one by one has none. Nobody handed over a shape to keep
    /// there, so its result belongs beside it rather than under a folder that
    /// was never mentioned.
    #[must_use]
    pub fn base_of(&self, file: &Path) -> Option<&Path> {
        self.roots
            .iter()
            .filter(|root| file.starts_with(root))
            // Roots can sit inside one another — `convert ~/Music ~/Music/Sets`
            // — and the nearer one is the shape the person had in mind.
            .max_by_key(|root| root.components().count())
            .map(PathBuf::as_path)
    }
}

fn sweep(directory: &Path, previous: PreviousOutput, into: &mut Found) {
    let is_previous_output = directory
        .file_name()
        .is_some_and(|name| name == OUTPUT_FOLDER);

    if is_previous_output && previous == PreviousOutput::Skip {
        return;
    }

    let Ok(entries) = std::fs::read_dir(directory) else {
        into.unreadable.push(directory.to_path_buf());
        return;
    };

    // A folder can open and still not give up all of it, and a track lost that
    // way is as absent as one behind a folder that would not open at all.
    let mut partial = false;

    for entry in entries {
        let Ok(entry) = entry else {
            partial = true;
            continue;
        };

        // read_dir's own file type, which does not follow symlinks — unlike
        // Path::is_dir, which does. A folder holding a link back to one above
        // it is an ordinary thing to have, and following it means descending
        // the same folders forever until the stack runs out.
        //
        // Only the descent is guarded. A link to a *file* cannot loop, and
        // somebody who keeps their library as links to tracks elsewhere means
        // those tracks, so it still falls through to the audio check below.
        let Ok(kind) = entry.file_type() else {
            partial = true;
            continue;
        };
        let path = entry.path();

        if kind.is_dir() {
            sweep(&path, previous, into);
        } else if is_audio(&path) {
            into.files.push(path);
        }
    }

    if partial {
        into.unreadable.push(directory.to_path_buf());
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
/// `base` is the folder the input was swept out of, when it came from one. A
/// folder handed over whole keeps its shape: one [`OUTPUT_FOLDER`] is made
/// beside it and each track comes out at the depth it went in at. Without that,
/// a folder tree grows an output folder at every level and the results of one
/// conversion end up scattered through it, with no single folder holding them
/// — somebody opening the obvious one finds tracks missing and no sign of
/// where they went.
///
/// A file named on its own has no `base`, and its result sits beside it.
///
/// # Errors
///
/// Fails when the input has no file name, and when the result would land on top
/// of the source.
pub fn output_path(
    input: &Path,
    into: Option<&Path>,
    base: Option<&Path>,
    codec: Codec,
) -> Result<PathBuf, PathError> {
    let stem = input
        .file_stem()
        .ok_or_else(|| PathError::Nameless(input.to_path_buf()))?;

    let holding = input.parent().unwrap_or(Path::new("."));
    let kept = base.and_then(|base| holding.strip_prefix(base).ok());

    let root = match (into, base) {
        (Some(directory), _) => directory.to_path_buf(),
        (None, Some(base)) => base.join(OUTPUT_FOLDER),
        (None, None) => holding.join(OUTPUT_FOLDER),
    };

    let mut destination = root.join(kept.unwrap_or(Path::new(""))).join(stem);
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
            .files
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
                .files
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };

        assert_eq!(names(PreviousOutput::Skip), ["track.wav"]);
        assert_eq!(names(PreviousOutput::Include), ["track.mp3", "track.wav"]);
    }

    /// A folder holding a link back to one above it is an ordinary thing to
    /// have, and following it walks the same folders again under a longer name
    /// each time. It stops when the paths grow past what the system will open,
    /// so nothing crashes — one track is simply found thirty-three times, and
    /// converting it writes it out thirty-three times into `_transcrate`
    /// folders scattered down the loop.
    #[cfg(unix)]
    #[test]
    fn a_link_back_up_the_tree_is_not_followed() {
        let dir = scratch("collect-symlink-loop");
        std::fs::create_dir_all(dir.join("Music/Sets")).expect("subdir");
        std::fs::write(dir.join("Music/track.wav"), b"").expect("write");
        std::os::unix::fs::symlink("../..", dir.join("Music/Sets/back")).expect("symlink");

        let found = collect(&[dir.join("Music")], PreviousOutput::Skip);

        assert_eq!(found.files.len(), 1, "found: {found:?}");
        assert!(found.files[0].ends_with("track.wav"));
    }

    /// Only the descent is guarded. Somebody who keeps a library as links to
    /// tracks held elsewhere means those tracks.
    #[cfg(unix)]
    #[test]
    fn a_link_to_a_track_is_still_a_track() {
        let dir = scratch("collect-symlink-file");
        std::fs::create_dir_all(dir.join("Library")).expect("subdir");
        std::fs::write(dir.join("real.wav"), b"").expect("write");
        std::os::unix::fs::symlink(dir.join("real.wav"), dir.join("Library/linked.wav"))
            .expect("symlink");

        let found = collect(&[dir.join("Library")], PreviousOutput::Skip);

        assert_eq!(found.files.len(), 1, "found: {found:?}");
        assert!(found.files[0].ends_with("linked.wav"));
    }

    /// A folder written by another account, or a drive that comes loose part
    /// way through, used to end the sweep of that branch in silence: the tracks
    /// behind it were never planned, never converted, and never mentioned.
    #[cfg(unix)]
    #[test]
    fn a_folder_that_will_not_open_is_named_rather_than_skipped() {
        use std::os::unix::fs::PermissionsExt;

        let dir = scratch("collect-unreadable");
        std::fs::write(dir.join("visible.wav"), b"").expect("write");

        let shut = dir.join("shut");
        std::fs::create_dir(&shut).expect("subdir");
        std::fs::write(shut.join("hidden.wav"), b"").expect("write");
        std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o000))
            .expect("close the folder");

        // Root ignores the bit, and CI containers often are root. There the
        // folder opens and there is nothing here to test.
        if std::fs::read_dir(&shut).is_ok() {
            std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755))
                .expect("reopen");
            return;
        }

        let found = collect(std::slice::from_ref(&dir), PreviousOutput::Skip);
        std::fs::set_permissions(&shut, std::fs::Permissions::from_mode(0o755)).expect("reopen");

        assert_eq!(found.files.len(), 1, "what was reached is still found");
        assert_eq!(found.unreadable.len(), 1);
        assert!(found.unreadable[0].ends_with("shut"));
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
            collect(std::slice::from_ref(&odd), PreviousOutput::Skip).files,
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
            .files
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
        let output =
            output_path(Path::new("/music/track.flac"), None, None, Codec::Mp3).expect("path");

        assert_eq!(output, Path::new("/music/_transcrate/track.mp3"));
    }

    /// A named folder is used as given: this is what a GUI's "save into" is.
    #[test]
    fn a_named_folder_is_used_as_given() {
        let output = output_path(
            Path::new("/music/track.flac"),
            Some(Path::new("/stick/DJ")),
            None,
            Codec::PcmAiff,
        )
        .expect("path");

        assert_eq!(output, Path::new("/stick/DJ/track.aiff"));
    }

    /// The failure this exists for: a folder handed over whole used to grow an
    /// output folder at every level it had, so a run over `Set B` left two
    /// tracks in `Set B/_transcrate` and a third somewhere five folders down.
    /// Opening the obvious folder showed a conversion with tracks missing.
    #[test]
    fn a_swept_folder_keeps_its_shape_under_one_output_folder() {
        let base = Path::new("/music/Set B");

        assert_eq!(
            output_path(
                Path::new("/music/Set B/near.wav"),
                None,
                Some(base),
                Codec::Mp3
            )
            .expect("path"),
            Path::new("/music/Set B/_transcrate/near.mp3"),
        );

        assert_eq!(
            output_path(
                Path::new("/music/Set B/2024/Live/TooDeep/far.wav"),
                None,
                Some(base),
                Codec::Mp3
            )
            .expect("path"),
            Path::new("/music/Set B/_transcrate/2024/Live/TooDeep/far.mp3"),
        );
    }

    /// The shape is kept under a named folder too, which is also what stops two
    /// albums holding `01.wav` from claiming one destination.
    #[test]
    fn a_named_folder_holds_the_same_shape() {
        let output = output_path(
            Path::new("/music/Set B/2024/far.wav"),
            Some(Path::new("/stick/DJ")),
            Some(Path::new("/music/Set B")),
            Codec::Mp3,
        )
        .expect("path");

        assert_eq!(output, Path::new("/stick/DJ/2024/far.mp3"));
    }

    /// Converting an MP3 into an MP3 in the folder it already sits in would
    /// destroy it, and the source is the one copy nobody has a backup of.
    #[test]
    fn writing_over_the_source_is_refused() {
        let refused = output_path(
            Path::new("/music/track.mp3"),
            Some(Path::new("/music")),
            None,
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
