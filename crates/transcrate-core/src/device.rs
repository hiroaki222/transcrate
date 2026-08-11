//! What each supported CDJ/XDJ player can actually play.
//!
//! Every value here comes from the manufacturer's own operating instructions;
//! `docs/device-compatibility.md` records the document behind each row. Nothing
//! goes in this table without one.
//!
//! Three properties of the real hardware shape the model:
//!
//! 1. **Sample-rate limits are per format, not per device.** A CDJ-3000 plays
//!    96 kHz FLAC but only 48 kHz MP3, so a single `max_sample_rate` on the
//!    device would be wrong for half of its formats.
//! 2. **`.m4a` is ambiguous.** It carries either AAC or ALAC, and the players
//!    that reject ALAC accept AAC, so compatibility has to be decided from the
//!    codec found in the stream rather than from the file extension.
//! 3. **Support is not always yes or no.** Some manuals omit a property
//!    entirely, and for the XDJ-XZ the manual and the support articles
//!    contradict each other, so those cases stay distinguishable.
//!
//! # Scope
//!
//! The tables describe the range worth *converting into*, not every stream a
//! player will accept. Sampling rates at or below 24 kHz — the MPEG-2 Layer-3
//! and low-rate AAC rows some manuals list — are omitted, because no DJ-facing
//! conversion targets them. USB is assumed throughout: CD and SD media carry
//! extra restrictions that this program does not model.

use serde::{Deserialize, Serialize};

/// Audio codec, as identified from the stream itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Codec {
    Mp3,
    /// AAC Low Complexity. No player lists any other AAC profile.
    AacLc,
    Alac,
    Flac,
    /// Uncompressed PCM in a RIFF/WAVE container.
    PcmWav,
    /// Uncompressed PCM in an AIFF container.
    PcmAiff,
}

/// Whether a device supports something, including the cases the documentation
/// leaves open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Support {
    Yes,
    No,
    /// The manual says nothing either way. Distinct from `No`: the behaviour is
    /// unverified rather than ruled out, and the user deserves to hear that.
    Unknown,
    /// Official sources contradict each other.
    Conflicting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileSystem {
    Fat16,
    Fat32,
    ExFat,
    HfsPlus,
    Ntfs,
}

/// Limits that apply only to lossy codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LossyLimits {
    /// Inclusive `(min, max)` in kbps.
    pub bitrate_kbps: (u16, u16),
    pub vbr: Support,
}

/// What one device accepts for one codec.
///
/// A device may list the same codec more than once when the limits differ
/// between the variants it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FormatSupport {
    pub codec: Codec,
    /// Sampling rates in Hz, as enumerated by the manufacturer. A rate missing
    /// from this list is outside the documented specification.
    pub sample_rates_hz: &'static [u32],
    pub bit_depths: &'static [u8],
    /// `None` for lossless codecs.
    pub lossy: Option<LossyLimits>,
}

/// One player, as far as file compatibility is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DeviceProfile {
    /// Stable kebab-case key used in profiles and on the command line.
    pub id: &'static str,
    pub display_name: &'static str,
    pub release_year: u16,
    pub formats: &'static [FormatSupport],
    pub filesystems: &'static [(FileSystem, Support)],
    /// Files nested deeper than this are not playable.
    pub max_folder_depth: u8,
    /// Entries beyond this are not listed by the browser.
    pub max_files_per_folder: Option<u32>,
    /// Longest artwork edge in pixels; larger images are not displayed. `None`
    /// where the manual states no limit.
    pub max_artwork_px: Option<u32>,
}

impl DeviceProfile {
    /// Support entries for `codec`, which may be empty if the player cannot
    /// play it at all.
    pub fn formats_for(&self, codec: Codec) -> impl Iterator<Item = &FormatSupport> {
        self.formats.iter().filter(move |f| f.codec == codec)
    }

    pub fn filesystem_support(&self, fs: FileSystem) -> Support {
        self.filesystems
            .iter()
            .find(|(candidate, _)| *candidate == fs)
            .map_or(Support::Unknown, |(_, support)| *support)
    }
}

/// Look up a player by its stable id.
pub fn by_id(id: &str) -> Option<&'static DeviceProfile> {
    DEVICES.iter().find(|d| d.id == id)
}

const RATES_STD: &[u32] = &[44_100, 48_000];
/// Players up to the 2021 generation also accept 32 kHz.
const RATES_WITH_32K: &[u32] = &[32_000, 44_100, 48_000];
const RATES_HIRES: &[u32] = &[44_100, 48_000, 88_200, 96_000];

const DEPTHS_PCM: &[u8] = &[16, 24];
/// Every player lists MP3 and AAC as 16-bit only.
const DEPTHS_LOSSY: &[u8] = &[16];

const fn mp3(sample_rates_hz: &'static [u32], vbr: Support) -> FormatSupport {
    FormatSupport {
        codec: Codec::Mp3,
        sample_rates_hz,
        bit_depths: DEPTHS_LOSSY,
        lossy: Some(LossyLimits {
            bitrate_kbps: (32, 320),
            vbr,
        }),
    }
}

const fn aac(sample_rates_hz: &'static [u32], vbr: Support) -> FormatSupport {
    FormatSupport {
        codec: Codec::AacLc,
        sample_rates_hz,
        bit_depths: DEPTHS_LOSSY,
        lossy: Some(LossyLimits {
            bitrate_kbps: (16, 320),
            vbr,
        }),
    }
}

const fn lossless(codec: Codec, sample_rates_hz: &'static [u32]) -> FormatSupport {
    FormatSupport {
        codec,
        sample_rates_hz,
        bit_depths: DEPTHS_PCM,
        lossy: None,
    }
}

/// CDJ-3000, CDJ-3000X, XDJ-AZ and OPUS-QUAD: lossy capped at 48 kHz, all four
/// lossless formats up to 96 kHz. Their published tables are identical.
///
/// None of these manuals state whether VBR is accepted; the CBR/VBR column that
/// earlier models carried was dropped rather than answered.
const FORMATS_HIRES: &[FormatSupport] = &[
    mp3(RATES_STD, Support::Unknown),
    aac(RATES_STD, Support::Unknown),
    lossless(Codec::PcmWav, RATES_HIRES),
    lossless(Codec::PcmAiff, RATES_HIRES),
    lossless(Codec::Alac, RATES_HIRES),
    lossless(Codec::Flac, RATES_HIRES),
];

/// XDJ-AN and OMNIS-DUO: the same four lossless formats as above, but capped at
/// 48 kHz. Being newer does not mean accepting more here.
const FORMATS_LOSSLESS_48K: &[FormatSupport] = &[
    mp3(RATES_STD, Support::Unknown),
    aac(RATES_STD, Support::Unknown),
    lossless(Codec::PcmWav, RATES_STD),
    lossless(Codec::PcmAiff, RATES_STD),
    lossless(Codec::Alac, RATES_STD),
    lossless(Codec::Flac, RATES_STD),
];

/// CDJ-2000NXS2: hi-res lossless like the 2020+ generation, but with the older
/// lossy table that still includes 32 kHz.
const FORMATS_NXS2: &[FormatSupport] = &[
    mp3(RATES_WITH_32K, Support::Yes),
    aac(RATES_WITH_32K, Support::Yes),
    lossless(Codec::PcmWav, RATES_HIRES),
    lossless(Codec::PcmAiff, RATES_HIRES),
    lossless(Codec::Alac, RATES_HIRES),
    lossless(Codec::Flac, RATES_HIRES),
];

/// XDJ-RX3 and XDJ-XZ: FLAC yes, ALAC no. On the XZ, FLAC arrived in firmware
/// 1.10 rather than at launch.
const FORMATS_FLAC_NO_ALAC: &[FormatSupport] = &[
    mp3(RATES_WITH_32K, Support::Yes),
    aac(RATES_WITH_32K, Support::Yes),
    lossless(Codec::PcmWav, RATES_STD),
    lossless(Codec::PcmAiff, RATES_STD),
    lossless(Codec::Flac, RATES_STD),
];

/// XDJ-RR: uncompressed PCM only. No FLAC, no ALAC, in any firmware — the word
/// "FLAC" does not appear anywhere in its manual.
const FORMATS_PCM_ONLY: &[FormatSupport] = &[
    mp3(RATES_WITH_32K, Support::Yes),
    aac(RATES_WITH_32K, Support::Yes),
    lossless(Codec::PcmWav, RATES_STD),
    lossless(Codec::PcmAiff, RATES_STD),
];

const FS_WITH_EXFAT: &[(FileSystem, Support)] = &[
    (FileSystem::Fat16, Support::Yes),
    (FileSystem::Fat32, Support::Yes),
    (FileSystem::ExFat, Support::Yes),
    (FileSystem::HfsPlus, Support::Yes),
    (FileSystem::Ntfs, Support::No),
];

const FS_NO_EXFAT: &[(FileSystem, Support)] = &[
    (FileSystem::Fat16, Support::Yes),
    (FileSystem::Fat32, Support::Yes),
    (FileSystem::ExFat, Support::No),
    (FileSystem::HfsPlus, Support::Yes),
    (FileSystem::Ntfs, Support::No),
];

/// XDJ-XZ only: its manual rules exFAT out, while two support articles published
/// later list it as supported. Neither has been retracted.
const FS_EXFAT_DISPUTED: &[(FileSystem, Support)] = &[
    (FileSystem::Fat16, Support::Yes),
    (FileSystem::Fat32, Support::Yes),
    (FileSystem::ExFat, Support::Conflicting),
    (FileSystem::HfsPlus, Support::Yes),
    (FileSystem::Ntfs, Support::No),
];

/// Every player this program checks against, newest first.
pub const DEVICES: &[DeviceProfile] = &[
    DeviceProfile {
        id: "xdj-an",
        display_name: "XDJ-AN",
        release_year: 2026,
        formats: FORMATS_LOSSLESS_48K,
        filesystems: FS_WITH_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: Some(800),
    },
    DeviceProfile {
        id: "cdj-3000x",
        display_name: "CDJ-3000X",
        release_year: 2025,
        formats: FORMATS_HIRES,
        filesystems: FS_WITH_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: None,
    },
    DeviceProfile {
        id: "xdj-az",
        display_name: "XDJ-AZ",
        release_year: 2025,
        formats: FORMATS_HIRES,
        filesystems: FS_WITH_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: None,
    },
    DeviceProfile {
        id: "omnis-duo",
        display_name: "OMNIS-DUO",
        release_year: 2024,
        formats: FORMATS_LOSSLESS_48K,
        filesystems: FS_WITH_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: Some(800),
    },
    DeviceProfile {
        id: "opus-quad",
        display_name: "OPUS-QUAD",
        release_year: 2023,
        formats: FORMATS_HIRES,
        filesystems: FS_WITH_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: None,
    },
    DeviceProfile {
        id: "xdj-rx3",
        display_name: "XDJ-RX3",
        release_year: 2021,
        formats: FORMATS_FLAC_NO_ALAC,
        filesystems: FS_WITH_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: None,
    },
    DeviceProfile {
        id: "cdj-3000",
        display_name: "CDJ-3000",
        release_year: 2020,
        formats: FORMATS_HIRES,
        filesystems: FS_WITH_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: None,
    },
    DeviceProfile {
        id: "xdj-xz",
        display_name: "XDJ-XZ",
        release_year: 2019,
        formats: FORMATS_FLAC_NO_ALAC,
        filesystems: FS_EXFAT_DISPUTED,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: None,
    },
    DeviceProfile {
        id: "xdj-rr",
        display_name: "XDJ-RR",
        release_year: 2018,
        formats: FORMATS_PCM_ONLY,
        filesystems: FS_NO_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: Some(800),
    },
    DeviceProfile {
        id: "cdj-2000nxs2",
        display_name: "CDJ-2000NXS2",
        release_year: 2016,
        formats: FORMATS_NXS2,
        filesystems: FS_NO_EXFAT,
        max_folder_depth: 8,
        max_files_per_folder: Some(10_000),
        max_artwork_px: None,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_ids_are_unique() {
        let mut ids: Vec<_> = DEVICES.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count, "duplicate device id in DEVICES");
    }

    #[test]
    fn every_device_plays_mp3() {
        for device in DEVICES {
            assert!(
                device.formats_for(Codec::Mp3).next().is_some(),
                "{} lists no MP3 support",
                device.id
            );
        }
    }

    #[test]
    fn lossy_formats_carry_bitrate_limits() {
        for device in DEVICES {
            for format in device.formats {
                let is_lossy = matches!(format.codec, Codec::Mp3 | Codec::AacLc);
                assert_eq!(
                    is_lossy,
                    format.lossy.is_some(),
                    "{}: {:?} disagrees with its lossy limits",
                    device.id,
                    format.codec
                );
            }
        }
    }

    /// The pairing that motivates the whole table: same era, opposite limits.
    #[test]
    fn newer_is_not_always_more_capable() {
        let xdj_an = by_id("xdj-an").expect("xdj-an");
        let nxs2 = by_id("cdj-2000nxs2").expect("cdj-2000nxs2");

        let flac_rates = |d: &DeviceProfile| {
            d.formats_for(Codec::Flac)
                .next()
                .expect("flac")
                .sample_rates_hz
        };
        assert!(!flac_rates(xdj_an).contains(&96_000));
        assert!(flac_rates(nxs2).contains(&96_000));

        assert_eq!(xdj_an.filesystem_support(FileSystem::ExFat), Support::Yes);
        assert_eq!(nxs2.filesystem_support(FileSystem::ExFat), Support::No);
    }
}
