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
    /// The volume label — what it is called in Finder, and the only part of
    /// this anyone recognises their own stick by.
    pub name: String,
    /// `None` when the filesystem is one no player reads, which is worth
    /// saying rather than forcing into the nearest family.
    pub filesystem: Option<FileSystem>,
    /// The name the operating system gave it, kept for the cases this program
    /// does not recognise.
    pub reported_as: String,
    /// How much the drive holds, and how much of that is still free. Bytes,
    /// because deciding what unit to show it in belongs to whatever displays it.
    pub total_bytes: u64,
    pub free_bytes: u64,
}

/// Every drive that could be carried to a gig.
///
/// Removable ones only, which is what separates a stick from the machine's own
/// disk. Nothing else is filtered: a mounted disk image is removable too and
/// will appear, and hiding a volume somebody can see in their file manager
/// would be the worse surprise of the two.
pub fn drives() -> Vec<Drive> {
    let disks = sysinfo::Disks::new_with_refreshed_list();

    let mut found: Vec<Drive> = disks
        .iter()
        .filter(|disk| disk.is_removable())
        .map(describe)
        .collect();

    // read_dir order for disks: whatever the system holds them in, which would
    // make the list reorder itself between one look and the next.
    found.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    found
}

fn describe(disk: &sysinfo::Disk) -> Drive {
    let reported_as = disk.file_system().to_string_lossy().into_owned();

    Drive {
        mount_point: disk.mount_point().to_path_buf(),
        name: disk.name().to_string_lossy().into_owned(),
        filesystem: recognise(&reported_as),
        reported_as,
        total_bytes: disk.total_space(),
        free_bytes: disk.available_space(),
    }
}

/// Identify the drive holding `path`.
///
/// Returns `None` when nothing is mounted there, which on a Mac usually means
/// the stick was ejected between plugging it in and asking about it.
pub fn drive_at(path: &Path) -> Option<Drive> {
    // Mount points are absolute, and what someone types rarely is: `KOMORI`
    // from inside /Volumes, a `..`, a symlink. Resolving first also rules out
    // a path that is not there — without that, every mistyped name matches the
    // root mount and comes back described as the machine's own disk.
    let path = without_verbatim_prefix(path.canonicalize().ok()?);

    let disks = sysinfo::Disks::new_with_refreshed_list();

    // Several mount points can contain the same path — on macOS everything sits
    // under `/` as well as under its own volume — so the longest match is the
    // one actually holding it.
    let disk = disks
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())?;

    Some(describe(disk))
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

/// Drop the `\\?\` that Windows' `canonicalize` puts on the front.
///
/// Mount points there are reported as `C:\`, and a verbatim path never starts
/// with one, so the two would never compare. Everywhere else this is the
/// identity.
fn without_verbatim_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(stripped) = path.to_str().and_then(|text| text.strip_prefix(r"\\?\")) {
            return PathBuf::from(stripped);
        }
    }

    path
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

    /// Whatever is plugged into the machine running this, the list has to hold
    /// only removable volumes and stay in the same order between two looks —
    /// a picker that reshuffles itself is one you cannot click.
    #[test]
    fn the_list_is_removable_volumes_in_a_settled_order() {
        let once = drives();
        let twice = drives();

        assert_eq!(once, twice);
        assert!(
            once.windows(2)
                .all(|w| w[0].mount_point <= w[1].mount_point)
        );

        // The machine's own disk is not something anyone carries to a gig.
        assert!(!once.iter().any(|drive| drive.mount_point == Path::new("/")));

        // Every one of them has to be answerable by the single-drive path too,
        // or the picker would offer something the check cannot then look at.
        for drive in &once {
            assert_eq!(drive_at(&drive.mount_point).as_ref(), Some(drive));
        }
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

    /// Mount points are always absolute, and a path typed at a shell is often
    /// not: `transcrate usb KOMORI` from inside /Volumes is the obvious way to
    /// ask, and comparing it against `/Volumes/KOMORI` unresolved finds
    /// nothing.
    #[test]
    fn a_relative_path_still_finds_its_drive() {
        // Tests run from the crate root, so this exists and is relative.
        let relative = std::path::Path::new("src");
        assert!(
            relative.exists() && relative.is_relative(),
            "the test needs a relative path that exists"
        );

        assert!(
            drive_at(relative).is_some(),
            "a relative path found no drive"
        );
    }

    /// Windows canonicalises to `\\?\C:\…` while reporting mount points as
    /// `C:\`, so the prefix has to come off or nothing ever matches.
    #[cfg(windows)]
    #[test]
    fn a_verbatim_prefix_is_removed() {
        assert_eq!(
            without_verbatim_prefix(PathBuf::from(r"\\?\C:\Users")),
            PathBuf::from(r"C:\Users")
        );
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
