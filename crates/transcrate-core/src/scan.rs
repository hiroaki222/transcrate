//! What is actually on a drive.
//!
//! A stick can pass every filesystem test and still fail in the booth, because
//! the players put limits on the shape of what is written to it as well as on
//! the format. Folders nested too far and folders holding too much simply do not
//! appear in the browser — the drive mounts, the tracks are there, and the
//! player shows nothing. That is worth catching at home.

use std::path::{Path, PathBuf};

use crate::compat::{self, AudioSpec, Issue};
use crate::device::DeviceProfile;
use crate::files;
use crate::parallel;
use crate::probe;

/// What the players allow of the tree itself.
///
/// Both numbers live on [`DeviceProfile`], so a drive is judged against the
/// players actually being taken out rather than against a constant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// How many folder levels deep the browser will descend, counting the drive
    /// root as the first.
    pub folder_depth: u8,
    /// How many entries one folder may hold, or `None` where no player
    /// documents a limit.
    pub entries_per_folder: Option<u32>,
}

impl Limits {
    /// The strictest reading of what every one of `players` allows.
    ///
    /// A drive is carried to whichever player is in the booth, so the one that
    /// gives way first is the one that decides.
    ///
    /// Returns `None` when handed no players, because there is then nothing to
    /// be strict about.
    pub fn strictest_of(players: &[&'static DeviceProfile]) -> Option<Self> {
        let folder_depth = players.iter().map(|p| p.max_folder_depth).min()?;

        // A player that documents no limit cannot loosen one that does, so an
        // absent number is skipped rather than treated as unlimited.
        let entries_per_folder = players.iter().filter_map(|p| p.max_files_per_folder).min();

        Some(Self {
            folder_depth,
            entries_per_folder,
        })
    }
}

/// A folder holding more than the browser will list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crowded {
    pub folder: PathBuf,
    /// Subfolders plus audio files: what the browser would have to show.
    pub entries: u32,
}

/// The shape of what is on the drive, before anything has been read.
///
/// This is a walk of the directory tree only — no file is opened, so it comes
/// back immediately even on a full stick.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Contents {
    /// Every audio file found, in a stable order.
    pub tracks: Vec<PathBuf>,
    /// Files the players would not list at all: artwork, playlists, notes.
    pub other_files: usize,
    /// Folders walked, not counting the root.
    pub folders: usize,
    /// How far the tree actually goes, counting the root as the first level.
    pub deepest: u8,
    /// Folders past [`Limits::folder_depth`], each named once at the level where
    /// the browser would stop. What is inside them is not walked: the player
    /// cannot reach it, so counting it would only inflate the totals.
    pub unreachable: Vec<PathBuf>,
    /// Folders past [`Limits::entries_per_folder`].
    pub crowded: Vec<Crowded>,
}

/// Walk `root`, measuring it against `limits`.
///
/// Entries whose name begins with a dot are ignored throughout. macOS writes
/// `.Spotlight-V100` and `.fseventsd` to every stick it touches, and no player
/// lists them — counting them would report a limit broken on a drive that works.
///
/// Symbolic links are never followed. They cannot exist on the filesystems the
/// players read, and following one is how a walk finds a loop and never returns.
pub fn walk(root: &Path, limits: Limits) -> Contents {
    let mut contents = Contents::default();
    descend(root, 1, limits, &mut contents);
    contents.tracks.sort();
    contents
}

fn descend(directory: &Path, level: u8, limits: Limits, contents: &mut Contents) {
    contents.deepest = contents.deepest.max(level);

    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    let mut listed = 0u32;
    let mut subfolders = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }

        // read_dir's own file type, which does not follow symlinks — unlike
        // Path::is_dir, which does.
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();

        if kind.is_dir() {
            listed += 1;
            subfolders.push(path);
        } else if kind.is_file() {
            if files::is_audio(&path) {
                listed += 1;
                contents.tracks.push(path);
            } else {
                contents.other_files += 1;
            }
        }
    }

    if limits.entries_per_folder.is_some_and(|most| listed > most) {
        contents.crowded.push(Crowded {
            folder: directory.to_path_buf(),
            entries: listed,
        });
    }

    contents.folders += subfolders.len();

    // Sorted so that a report names the same folder first every time; read_dir
    // hands entries back in whatever order the filesystem holds them.
    subfolders.sort();

    for subfolder in subfolders {
        if level >= limits.folder_depth {
            // Not walked, but it is still a level: reporting the tree as ending
            // at the limit would contradict the very line naming what is past it.
            contents.deepest = contents.deepest.max(level + 1);
            contents.unreachable.push(subfolder);
        } else {
            descend(&subfolder, level + 1, limits, contents);
        }
    }
}

/// One track, read and judged.
#[derive(Debug, Clone)]
pub struct Verdict {
    pub path: PathBuf,
    /// What the file turned out to be, or `None` if it could not be read.
    pub spec: Option<AudioSpec>,
    /// Why each player named will refuse it. Empty when every one will play it.
    pub refused_by: Vec<Refusal>,
    /// Why the file could not be read, if it could not be.
    pub error: Option<String>,
}

/// One player's reasons for refusing one track.
#[derive(Debug, Clone)]
pub struct Refusal {
    pub player: &'static DeviceProfile,
    pub issues: Vec<Issue>,
}

impl Verdict {
    /// Whether every player named will play this track.
    pub fn plays(&self) -> bool {
        self.error.is_none() && self.refused_by.is_empty()
    }
}

/// Read every track and work out which of `players` will play it.
///
/// One ffprobe process per track, `concurrency` of them at once. `on_finished`
/// is called as each lands, because a full stick is thousands of files and the
/// count has to move while it works.
///
/// # Panics
///
/// Panics if a worker thread panics.
pub fn judge(
    tracks: &[PathBuf],
    ffprobe: &Path,
    players: &[&'static DeviceProfile],
    concurrency: usize,
    on_finished: &(dyn Fn(usize) + Sync),
) -> Vec<Verdict> {
    parallel::map(
        tracks,
        concurrency,
        &|_, track| match probe::run(ffprobe, track) {
            Ok(spec) => Verdict {
                path: track.clone(),
                refused_by: refusals(&spec, players),
                spec: Some(spec),
                error: None,
            },
            Err(error) => Verdict {
                path: track.clone(),
                spec: None,
                refused_by: Vec::new(),
                error: Some(error.to_string()),
            },
        },
        &|index, _| on_finished(index),
    )
}

fn refusals(spec: &AudioSpec, players: &[&'static DeviceProfile]) -> Vec<Refusal> {
    players
        .iter()
        .filter_map(|player| {
            let issues = compat::check(spec, player);
            (!issues.is_empty()).then_some(Refusal { player, issues })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DEVICES;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("transcrate-scan-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    fn nest(root: &Path, levels: usize) -> PathBuf {
        let mut path = root.to_path_buf();
        for level in 0..levels {
            path = path.join(format!("level-{level}"));
        }
        std::fs::create_dir_all(&path).expect("create nesting");
        path
    }

    const EVERY_PLAYER: Limits = Limits {
        folder_depth: 8,
        entries_per_folder: Some(10_000),
    };

    /// The drive root is the first level, so eight levels leaves seven folders
    /// of nesting below it. This is the stricter of the two readings, and being
    /// wrong the other way means telling somebody a working stick is fine when
    /// the booth will show them an empty folder.
    #[test]
    fn nesting_is_counted_from_the_root() {
        let dir = scratch("depth");
        std::fs::write(nest(&dir, 7).join("deep.wav"), b"").expect("write");

        let contents = walk(&dir, EVERY_PLAYER);

        assert_eq!(contents.deepest, 8);
        assert!(contents.unreachable.is_empty());
        assert_eq!(contents.tracks.len(), 1);
    }

    /// The player stops at the limit, so nothing below it is reachable — and
    /// walking on would count tracks nobody can select.
    #[test]
    fn a_folder_past_the_limit_is_named_and_not_walked() {
        let dir = scratch("too-deep");
        std::fs::write(nest(&dir, 8).join("lost.wav"), b"").expect("write");

        let contents = walk(&dir, EVERY_PLAYER);

        assert_eq!(contents.unreachable.len(), 1);
        assert!(contents.unreachable[0].ends_with("level-7"));
        assert!(contents.tracks.is_empty());
        // Saying the tree ends at 8 would contradict the folder named as past it.
        assert_eq!(contents.deepest, 9);
    }

    /// A player lists folders and the tracks it can play. Twenty thousand JPEGs
    /// in one folder is not a browser problem, and reporting it as one would
    /// send somebody looking for a fault that is not there.
    #[test]
    fn only_what_the_browser_lists_counts_towards_the_limit() {
        let dir = scratch("crowded");
        let limits = Limits {
            folder_depth: 8,
            entries_per_folder: Some(2),
        };

        for name in ["a.wav", "b.wav", "c.jpg", "d.jpg", "e.txt"] {
            std::fs::write(dir.join(name), b"").expect("write");
        }

        assert!(walk(&dir, limits).crowded.is_empty());

        std::fs::write(dir.join("f.wav"), b"").expect("write");
        let contents = walk(&dir, limits);

        assert_eq!(contents.crowded.len(), 1);
        assert_eq!(contents.crowded[0].entries, 3);
        assert_eq!(contents.other_files, 3);
    }

    /// macOS writes these to every stick it touches. No player lists them, so
    /// counting them would report a limit broken on a drive that works.
    #[test]
    fn what_the_operating_system_leaves_behind_is_ignored() {
        let dir = scratch("hidden");
        std::fs::create_dir_all(dir.join(".Spotlight-V100/deep")).expect("create");
        std::fs::write(dir.join(".DS_Store"), b"").expect("write");
        std::fs::write(dir.join("track.wav"), b"").expect("write");

        let contents = walk(&dir, EVERY_PLAYER);

        assert_eq!(contents.folders, 0);
        assert_eq!(contents.other_files, 0);
        assert_eq!(contents.tracks.len(), 1);
    }

    /// Every player currently documents the same two numbers, but the drive has
    /// to be judged against the ones being taken out, not against a constant.
    #[test]
    fn the_first_player_to_give_way_sets_the_limit() {
        let every: Vec<_> = DEVICES.iter().collect();

        assert_eq!(Limits::strictest_of(&every), Some(EVERY_PLAYER));
        assert_eq!(Limits::strictest_of(&[]), None);
    }
}
