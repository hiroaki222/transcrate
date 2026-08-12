//! What a player will make of a stick before it is carried to a gig.
//!
//! Read-only throughout. Nothing here writes to a drive, formats one, or moves
//! a file: a tool that inspects someone's set on a Friday evening has no
//! business being able to damage it.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::device::{DeviceProfile, FileSystem, Support};

/// Which players will read a drive, and which will not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Readers {
    pub readable: Vec<&'static DeviceProfile>,
    /// Each player that will not, with the verdict that put it here — a
    /// documented refusal and a contradiction between sources are different
    /// things to be told.
    pub unreadable: Vec<(&'static DeviceProfile, Support)>,
}

/// What was found at a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Drive {
    pub mount_point: PathBuf,
    /// `None` when the filesystem is one no player reads, which is worth
    /// saying rather than forcing into the nearest family.
    pub filesystem: Option<FileSystem>,
    /// The name the operating system gave it, kept for the cases this program
    /// does not recognise.
    pub reported_as: String,
}

/// Identify the drive holding `path`.
///
/// Returns `None` when nothing is mounted there, which on a Mac usually means
/// the stick was ejected between plugging it in and asking about it.
pub fn drive_at(path: &Path) -> Option<Drive> {
    // Every path starts with `/`, so a mistyped one matches the root mount and
    // would come back described as the machine's own disk.
    if !path.exists() {
        return None;
    }

    let disks = sysinfo::Disks::new_with_refreshed_list();

    // Several mount points can contain the same path — on macOS everything sits
    // under `/` as well as under its own volume — so the longest match is the
    // one actually holding it.
    let disk = disks
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())?;

    let reported_as = disk.file_system().to_string_lossy().into_owned();

    Some(Drive {
        mount_point: disk.mount_point().to_path_buf(),
        filesystem: recognise(&reported_as),
        reported_as,
    })
}

/// Split players by whether they will read this filesystem.
pub fn readers_of(filesystem: FileSystem, players: &'static [DeviceProfile]) -> Readers {
    let mut readable = Vec::new();
    let mut unreadable = Vec::new();

    for player in players {
        match player.filesystem_support(filesystem) {
            Support::Yes => readable.push(player),
            other => unreadable.push((player, other)),
        }
    }

    Readers {
        readable,
        unreadable,
    }
}

/// The filesystem family behind an operating system's name for it.
///
/// Each platform names these differently, and macOS uses one name for every
/// FAT variant — a player only cares which family it belongs to. Anything not
/// listed is left unrecognised rather than forced into the nearest match: a
/// wrong guess here would clear a drive no player can read.
fn recognise(reported: &str) -> Option<FileSystem> {
    match reported.to_ascii_lowercase().as_str() {
        "exfat" => Some(FileSystem::ExFat),
        // macOS reports every FAT as msdos, Linux as vfat.
        "msdos" | "vfat" | "fat" | "fat32" => Some(FileSystem::Fat32),
        "fat16" => Some(FileSystem::Fat16),
        "hfs" | "hfsplus" | "hfs+" | "apple_hfs" => Some(FileSystem::HfsPlus),
        "ntfs" | "ntfs3" => Some(FileSystem::Ntfs),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{FileSystem, by_id};

    /// Every platform has its own name for the same filesystem, and macOS uses
    /// one name for every FAT variant. A player only cares which family it is.
    #[test]
    fn the_name_each_platform_uses_is_recognised() {
        assert_eq!(recognise("exfat"), Some(FileSystem::ExFat));
        assert_eq!(recognise("exFAT"), Some(FileSystem::ExFat));

        assert_eq!(recognise("msdos"), Some(FileSystem::Fat32));
        assert_eq!(recognise("vfat"), Some(FileSystem::Fat32));
        assert_eq!(recognise("FAT32"), Some(FileSystem::Fat32));

        assert_eq!(recognise("hfs"), Some(FileSystem::HfsPlus));
        assert_eq!(recognise("apfs_hfs"), None);

        assert_eq!(recognise("ntfs"), Some(FileSystem::Ntfs));
        assert_eq!(recognise("NTFS"), Some(FileSystem::Ntfs));
    }

    /// A Mac's own disk is not something any player reads, and reporting it as
    /// unknown is more honest than guessing a family for it.
    #[test]
    fn a_filesystem_no_player_reads_is_not_guessed_at() {
        assert_eq!(recognise("apfs"), None);
        assert_eq!(recognise("ext4"), None);
        assert_eq!(recognise("btrfs"), None);
    }

    /// Every path starts with `/`, so a mistyped one would otherwise match the
    /// root and be reported as the machine's own disk — an answer about a
    /// drive that was never there.
    #[test]
    fn a_path_that_is_not_there_is_not_a_drive() {
        let missing = std::path::Path::new("/transcrate-no-such-volume-here");
        assert!(
            !missing.exists(),
            "the test needs a path that does not exist"
        );

        assert_eq!(drive_at(missing), None);
    }

    /// The split is the whole point: which machines in the booth will read this
    /// stick, and which will not.
    #[test]
    fn players_are_split_by_whether_they_read_the_filesystem() {
        let split = readers_of(FileSystem::ExFat, crate::device::DEVICES);

        let reads = |id: &str| split.readable.iter().any(|player| player.id == id);
        let refuses = |id: &str| split.unreadable.iter().any(|(player, _)| player.id == id);

        assert!(reads("cdj-3000"), "the 3000 reads exFAT");
        // The two that do not, and the reason this check exists at all.
        assert!(refuses("cdj-2000nxs2"), "the NXS2 does not");
        assert!(refuses("xdj-rr"), "nor does the RR");

        assert_eq!(
            split.readable.len() + split.unreadable.len(),
            crate::device::DEVICES.len(),
            "every player has to land on one side"
        );
    }

    /// NTFS is refused by every player, which is worth saying plainly rather
    /// than as ten separate lines.
    #[test]
    fn ntfs_is_refused_by_everything() {
        let split = readers_of(FileSystem::Ntfs, crate::device::DEVICES);

        assert!(split.readable.is_empty(), "{:?}", split.readable);
        assert_eq!(split.unreadable.len(), crate::device::DEVICES.len());
    }

    /// The XDJ-XZ's own documentation disagrees with itself about exFAT, and a
    /// stick is exactly where that matters. It is reported as its own case
    /// rather than folded into yes or no.
    #[test]
    fn a_disputed_filesystem_is_reported_as_disputed() {
        let split = readers_of(FileSystem::ExFat, crate::device::DEVICES);
        let xz = by_id("xdj-xz").expect("xdj-xz");

        let verdict = split
            .unreadable
            .iter()
            .find(|(player, _)| player.id == xz.id)
            .map(|(_, support)| *support);

        assert_eq!(verdict, Some(crate::device::Support::Conflicting));
    }
}
