//! The shapes the window receives.
//!
//! Nothing here decides anything: every judgement comes from `transcrate-core`,
//! and this module only arranges the answers for the wire. Wording is left to
//! the window, so the interface can be Japanese without translations living in
//! Rust.

use serde::Serialize;

use transcrate_core::compat::{self, AudioSpec, Issue};
use transcrate_core::device::{Codec, DeviceProfile, FileSystem, Support};

/// The six columns of the compatibility table, in the order they are shown.
pub(crate) const COLUMNS: [(&str, Codec); 6] = [
    ("mp3", Codec::Mp3),
    ("aac", Codec::AacLc),
    ("wav", Codec::PcmWav),
    ("aiff", Codec::PcmAiff),
    ("flac", Codec::Flac),
    ("alac", Codec::Alac),
];

/// One player's verdict on one track, sized to fit in a status lamp.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Lamp {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) short: &'static str,
    pub(crate) ok: bool,
    pub(crate) issues: Vec<Issue>,
}

/// Every player's verdict on `spec`, in the fixed table order.
///
/// The order is the point: the lamps line up into columns across a list, so a
/// player that fails everything shows up as a stripe rather than as a hundred
/// separate readings.
pub(crate) fn lamps_for(spec: &AudioSpec, players: &[&'static DeviceProfile]) -> Vec<Lamp> {
    players
        .iter()
        .map(|player| {
            let issues = compat::check(spec, player);
            Lamp {
                id: player.id,
                name: player.display_name,
                short: player.lamp_name,
                ok: issues.is_empty(),
                issues,
            }
        })
        .collect()
}

/// What a conversion will do to one file, and what it will be worth afterwards.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Track {
    pub(crate) path: String,
    pub(crate) name: String,
    /// `None` when the file could not be read, in which case `error` says why.
    pub(crate) source: Option<AudioSpec>,
    pub(crate) output: Option<AudioSpec>,
    pub(crate) output_path: Option<String>,
    pub(crate) dither: bool,
    /// Whether the source was already short of information. Nothing a
    /// conversion does can put back what its first encoder threw away, so this
    /// is the one figure on the screen that cannot be improved.
    pub(crate) thin: bool,
    /// Verdicts as the file stands.
    pub(crate) now: Vec<Lamp>,
    /// Verdicts on what the conversion would produce.
    pub(crate) after: Vec<Lamp>,
    pub(crate) error: Option<String>,
}

impl Track {
    /// A file that could not be read. Reported rather than dropped: a track
    /// missing from the list is worse than one that says why it failed.
    pub(crate) fn unreadable(path: &std::path::Path, error: String) -> Self {
        Self {
            path: path.display().to_string(),
            name: file_name(path),
            source: None,
            output: None,
            output_path: None,
            dither: false,
            thin: false,
            now: Vec::new(),
            after: Vec::new(),
            error: Some(error),
        }
    }
}

pub(crate) fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// One row of the compatibility table.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeviceRow {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) short: &'static str,
    pub(crate) year: u16,
    /// Highest documented rate per codec, in the order of [`COLUMNS`]. `None`
    /// where the player does not list the codec at all.
    pub(crate) rates_hz: Vec<Option<u32>>,
    pub(crate) exfat: bool,
    pub(crate) max_folder_depth: u8,
}

pub(crate) fn device_rows(players: &'static [DeviceProfile]) -> Vec<DeviceRow> {
    players
        .iter()
        .map(|player| DeviceRow {
            id: player.id,
            name: player.display_name,
            short: player.lamp_name,
            year: player.release_year,
            rates_hz: COLUMNS
                .iter()
                .map(|(_, codec)| highest_rate(player, *codec))
                .collect(),
            // Anything short of a documented yes is treated as a no. A drive
            // that turns out unreadable in the booth cannot be fixed there.
            exfat: player.filesystem_support(FileSystem::ExFat) == Support::Yes,
            max_folder_depth: player.max_folder_depth,
        })
        .collect()
}

fn highest_rate(player: &DeviceProfile, codec: Codec) -> Option<u32> {
    player
        .formats_for(codec)
        .filter_map(|format| format.sample_rates_hz.iter().max())
        .max()
        .copied()
}

/// What was found on a drive, and which players will read it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Drive {
    pub(crate) mount_point: String,
    /// The volume label — what it is called in Finder, and the only part of
    /// this anyone recognises their own stick by.
    pub(crate) name: String,
    /// `null` where the filesystem is one no player reads, which is worth
    /// saying rather than forcing into the nearest family.
    pub(crate) filesystem: Option<&'static str>,
    pub(crate) reported_as: String,
    pub(crate) lamps: Vec<Lamp>,
    pub(crate) readable: usize,
}

/// What is on the drive, measured against what the players allow.
///
/// Counts rather than the paths themselves, except where a path is the answer:
/// "eight folders too deep" is not actionable, and the folder's name is.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Contents {
    pub(crate) tracks: usize,
    pub(crate) folders: usize,
    pub(crate) other_files: usize,
    pub(crate) deepest: u8,
    /// The limits the drive was judged against, so the window can say which
    /// number was broken rather than hard-coding one.
    pub(crate) depth_limit: u8,
    pub(crate) entry_limit: Option<u32>,
    /// Folders the browser never reaches, named relative to the drive.
    pub(crate) unreachable: Vec<String>,
    pub(crate) crowded: Vec<Crowded>,
    /// Folders the walk itself could not list. Whatever is inside them is
    /// missing from every count above.
    pub(crate) unreadable: Vec<String>,
    /// Only the tracks at least one player refuses. A stick holds thousands and
    /// the ones that work need no attention.
    pub(crate) failing: Vec<FailingTrack>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Crowded {
    pub(crate) folder: String,
    pub(crate) entries: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FailingTrack {
    pub(crate) path: String,
    pub(crate) name: String,
    /// Where it sits on the drive: two tracks of the same name in different
    /// folders are otherwise indistinguishable.
    pub(crate) folder: String,
    pub(crate) spec: Option<AudioSpec>,
    pub(crate) lamps: Vec<Lamp>,
    pub(crate) error: Option<String>,
}

/// One drive as the picker shows it, before anything has been read off it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Mounted {
    pub(crate) mount_point: String,
    pub(crate) name: String,
    /// `null` where no player reads it; the window falls back to `reportedAs`.
    pub(crate) filesystem: Option<&'static str>,
    pub(crate) reported_as: String,
    /// How many of the chosen players read it, out of how many were chosen —
    /// the whole verdict on a drive, at the size a list row has room for.
    pub(crate) readable: usize,
    pub(crate) players: usize,
    pub(crate) total_bytes: u64,
    pub(crate) free_bytes: u64,
}

pub(crate) const fn filesystem_name(filesystem: FileSystem) -> &'static str {
    match filesystem {
        FileSystem::Fat16 => "FAT16",
        FileSystem::Fat32 => "FAT32",
        FileSystem::ExFat => "exFAT",
        FileSystem::HfsPlus => "HFS+",
        FileSystem::Ntfs => "NTFS",
    }
}

/// Whether ffmpeg and ffprobe can be run at all.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Tools {
    pub(crate) ffmpeg: bool,
    pub(crate) ffprobe: bool,
}

/// Progress of a long-running sweep, emitted as it goes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Progress {
    pub(crate) done: usize,
    pub(crate) total: usize,
    pub(crate) name: String,
}
