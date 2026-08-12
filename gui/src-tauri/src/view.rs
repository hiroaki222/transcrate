//! The shapes the window receives.
//!
//! Nothing here decides anything: every judgement comes from `transcrate-core`,
//! and this module only arranges the answers for the wire. Wording is left to
//! the window, so the interface can be Japanese without translations living in
//! Rust.

use serde::Serialize;

use transcrate_core::compat::{self, AudioSpec, Issue};
use transcrate_core::device::{Codec, DeviceProfile, FileSystem, Support};
use transcrate_core::plan::Action;

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
    /// `copy`, `retag` or `encode`.
    pub(crate) action: Option<&'static str>,
    pub(crate) dither: bool,
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
            action: None,
            dither: false,
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

pub(crate) const fn action_name(action: Action) -> &'static str {
    match action {
        Action::Copy => "copy",
        Action::Retag => "retag",
        Action::Encode { .. } => "encode",
    }
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
    /// `null` where the filesystem is one no player reads, which is worth
    /// saying rather than forcing into the nearest family.
    pub(crate) filesystem: Option<&'static str>,
    pub(crate) reported_as: String,
    pub(crate) lamps: Vec<Lamp>,
    pub(crate) readable: usize,
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
