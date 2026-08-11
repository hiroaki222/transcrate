//! Checking a planned output against what a player can accept.

use serde::Serialize;

use crate::device::{Codec, DeviceProfile};

/// The shape of an audio stream: either one read off an existing file or one a
/// conversion is about to produce. Both are checked the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AudioSpec {
    pub codec: Codec,
    pub sample_rate_hz: u32,
    /// `None` for lossy codecs, which every player lists as 16-bit only.
    pub bit_depth: Option<u8>,
    /// `None` for lossless codecs.
    pub bitrate_kbps: Option<u16>,
}

/// A reason one player will not play the planned output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Issue {
    /// The player's format table has no row for this codec.
    CodecUnsupported { codec: Codec },
    /// The codec is playable, but not at this sampling frequency.
    SampleRateUnsupported { codec: Codec, requested_hz: u32 },
    /// The codec is playable, but not at this bit depth.
    BitDepthUnsupported { codec: Codec, requested_bits: u8 },
    BitrateOutOfRange {
        codec: Codec,
        requested_kbps: u16,
        allowed_kbps: (u16, u16),
    },
}

/// Every reason `device` will refuse `spec`, or an empty list if it will play.
///
/// An unsupported codec short-circuits the rest: without a format row there is
/// nothing to compare a rate or a depth against, and listing consequences of
/// the same fact would bury the one that matters. Everything past that point is
/// independent — a track can be both too fast and too deep — so the remaining
/// checks all run and all report.
///
/// A player may list one codec under several rows with different limits, so a
/// value is acceptable when *any* row admits it.
pub fn check(spec: &AudioSpec, device: &DeviceProfile) -> Vec<Issue> {
    let formats: Vec<_> = device.formats_for(spec.codec).collect();
    if formats.is_empty() {
        return vec![Issue::CodecUnsupported { codec: spec.codec }];
    }

    let mut issues = Vec::new();

    if !formats
        .iter()
        .any(|f| f.sample_rates_hz.contains(&spec.sample_rate_hz))
    {
        issues.push(Issue::SampleRateUnsupported {
            codec: spec.codec,
            requested_hz: spec.sample_rate_hz,
        });
    }

    if let Some(requested_bits) = spec.bit_depth
        && !formats
            .iter()
            .any(|f| f.bit_depths.contains(&requested_bits))
    {
        issues.push(Issue::BitDepthUnsupported {
            codec: spec.codec,
            requested_bits,
        });
    }

    if let Some(requested_kbps) = spec.bitrate_kbps {
        let limits: Vec<_> = formats
            .iter()
            .filter_map(|f| f.lossy.map(|l| l.bitrate_kbps))
            .collect();
        let admitted = limits
            .iter()
            .any(|(min, max)| (*min..=*max).contains(&requested_kbps));

        if !limits.is_empty() && !admitted {
            let allowed_kbps = limits.iter().fold((u16::MAX, 0), |(lo, hi), &(min, max)| {
                (lo.min(min), hi.max(max))
            });
            issues.push(Issue::BitrateOutOfRange {
                codec: spec.codec,
                requested_kbps,
                allowed_kbps,
            });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{Codec, by_id};

    fn lossless(codec: Codec, sample_rate_hz: u32) -> AudioSpec {
        AudioSpec {
            codec,
            sample_rate_hz,
            bit_depth: Some(24),
            bitrate_kbps: None,
        }
    }

    /// The XDJ-RR has no FLAC row at all, so the codec itself is the problem —
    /// its sampling rate never gets a say.
    #[test]
    fn codec_absent_from_the_table_is_reported_once() {
        let rr = by_id("xdj-rr").expect("xdj-rr");
        let issues = check(&lossless(Codec::Flac, 96_000), rr);
        assert_eq!(issues, vec![Issue::CodecUnsupported { codec: Codec::Flac }]);
    }

    /// The XDJ-RX3 does play FLAC, but stops at 48 kHz. This is the case that
    /// a per-device "maximum sample rate" would get wrong.
    #[test]
    fn supported_codec_above_its_rate_limit_is_reported() {
        let rx3 = by_id("xdj-rx3").expect("xdj-rx3");
        let issues = check(&lossless(Codec::Flac, 96_000), rx3);
        assert_eq!(
            issues,
            vec![Issue::SampleRateUnsupported {
                codec: Codec::Flac,
                requested_hz: 96_000
            }]
        );
    }

    /// ALAC and AAC share the `.m4a` extension, and the XDJ-RX3 accepts only
    /// the latter. Deciding on the extension would let this through.
    #[test]
    fn alac_is_distinguished_from_aac_in_the_same_container() {
        let rx3 = by_id("xdj-rx3").expect("xdj-rx3");
        assert_eq!(
            check(&lossless(Codec::Alac, 44_100), rx3),
            vec![Issue::CodecUnsupported { codec: Codec::Alac }]
        );
        assert!(
            check(
                &AudioSpec {
                    codec: Codec::AacLc,
                    sample_rate_hz: 44_100,
                    bit_depth: None,
                    bitrate_kbps: Some(320),
                },
                rx3
            )
            .is_empty()
        );
    }

    /// Production files are routinely 32-bit float. No player takes more than
    /// 24, so this is the check that catches a track dragged straight out of a
    /// DAW.
    #[test]
    fn thirty_two_bit_is_rejected_by_every_player() {
        let spec = AudioSpec {
            codec: Codec::PcmWav,
            sample_rate_hz: 44_100,
            bit_depth: Some(32),
            bitrate_kbps: None,
        };
        for device in crate::device::DEVICES {
            assert_eq!(
                check(&spec, device),
                vec![Issue::BitDepthUnsupported {
                    codec: Codec::PcmWav,
                    requested_bits: 32
                }],
                "{} accepted 32-bit",
                device.id
            );
        }
    }

    #[test]
    fn bitrate_above_the_documented_ceiling_is_reported() {
        let cdj_3000 = by_id("cdj-3000").expect("cdj-3000");
        let spec = AudioSpec {
            codec: Codec::Mp3,
            sample_rate_hz: 44_100,
            bit_depth: None,
            bitrate_kbps: Some(400),
        };
        assert_eq!(
            check(&spec, cdj_3000),
            vec![Issue::BitrateOutOfRange {
                codec: Codec::Mp3,
                requested_kbps: 400,
                allowed_kbps: (32, 320),
            }]
        );
    }

    /// Rate and depth fail independently, and fixing one leaves the other, so
    /// both have to surface at once rather than one at a time.
    #[test]
    fn independent_faults_are_all_reported() {
        let rr = by_id("xdj-rr").expect("xdj-rr");
        let spec = AudioSpec {
            codec: Codec::PcmWav,
            sample_rate_hz: 96_000,
            bit_depth: Some(32),
            bitrate_kbps: None,
        };
        assert_eq!(
            check(&spec, rr),
            vec![
                Issue::SampleRateUnsupported {
                    codec: Codec::PcmWav,
                    requested_hz: 96_000
                },
                Issue::BitDepthUnsupported {
                    codec: Codec::PcmWav,
                    requested_bits: 32
                },
            ]
        );
    }

    /// The default profile has to be exactly that: safe on every player here.
    #[test]
    fn cdj_safe_mp3_clears_every_player() {
        let spec = AudioSpec {
            codec: Codec::Mp3,
            sample_rate_hz: 44_100,
            bit_depth: None,
            bitrate_kbps: Some(320),
        };
        for device in crate::device::DEVICES {
            assert!(
                check(&spec, device).is_empty(),
                "{} rejected the default profile: {:?}",
                device.id,
                check(&spec, device)
            );
        }
    }
}
