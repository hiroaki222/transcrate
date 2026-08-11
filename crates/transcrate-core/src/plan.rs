//! Deciding what a conversion will do, before anything runs.

use serde::{Deserialize, Serialize};

use crate::compat::AudioSpec;
use crate::device::Codec;

/// How the output's sampling rate follows from the source's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SampleRatePolicy {
    /// Whatever the source has.
    Preserve,
    /// Always this rate, resampling up or down to reach it.
    Fixed(u32),
    /// The source rate, unless it is above this.
    CapAt(u32),
}

/// How the output's bit depth follows from the source's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BitDepthPolicy {
    Preserve,
    Fixed(u8),
    CapAt(u8),
}

/// What to convert into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Target {
    pub codec: Codec,
    pub sample_rate: SampleRatePolicy,
    /// Ignored for lossy codecs, which carry no bit depth.
    pub bit_depth: BitDepthPolicy,
    /// Ignored for lossless codecs.
    pub bitrate_kbps: Option<u16>,
}

impl Target {
    /// Plays on every player in the table: MP3 at 320 kbps and 44.1 kHz.
    pub const CDJ_SAFE: Self = Self {
        codec: Codec::Mp3,
        sample_rate: SampleRatePolicy::Fixed(44_100),
        bit_depth: BitDepthPolicy::Preserve,
        bitrate_kbps: Some(320),
    };

    /// Lossless, and still playable everywhere. AIFF rather than WAV because
    /// WAV's tagging is a mess that players disagree about; the ceilings are
    /// the lowest any supported player accepts, so one file works on all of
    /// them.
    pub const LOSSLESS: Self = Self {
        codec: Codec::PcmAiff,
        sample_rate: SampleRatePolicy::CapAt(48_000),
        bit_depth: BitDepthPolicy::CapAt(24),
        bitrate_kbps: None,
    };

    /// For the copy you keep rather than the one you play: FLAC at whatever the
    /// source was. Makes no promise about any player.
    pub const ARCHIVE: Self = Self {
        codec: Codec::Flac,
        sample_rate: SampleRatePolicy::Preserve,
        bit_depth: BitDepthPolicy::Preserve,
        bitrate_kbps: None,
    };

    /// The built-in profiles, by the name used on the command line.
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "cdj-safe" => Some(Self::CDJ_SAFE),
            "lossless" => Some(Self::LOSSLESS),
            "archive" => Some(Self::ARCHIVE),
            _ => None,
        }
    }

    /// Every built-in profile name, for completion and error messages.
    pub const NAMES: [&'static str; 3] = ["cdj-safe", "lossless", "archive"];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "action")]
pub enum Action {
    /// The source already matches the target, so copy the bytes across. This is
    /// the cheapest outcome and worth reaching for: a library that is already
    /// in the right format should not be re-encoded to prove it.
    Copy,
    Encode {
        dither: bool,
    },
}

/// What one file's conversion will produce, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub output: AudioSpec,
    pub action: Action,
}

/// Work out the output and how to reach it.
pub fn plan(source: &AudioSpec, target: &Target) -> Plan {
    let output = resolve(source, target);

    let action = if output == *source {
        Action::Copy
    } else {
        Action::Encode {
            dither: shortens_word_length(source, &output),
        }
    };

    Plan { output, action }
}

/// The ffmpeg options that carry out an encode, to be placed after `-i INPUT`
/// and before the output path.
///
/// Meaningless for a [`Action::Copy`] plan, which is a file copy rather than an
/// ffmpeg run.
pub fn encode_args(plan: &Plan) -> Vec<String> {
    let output = &plan.output;
    let mut args = vec!["-map".to_owned(), "0:a:0".to_owned()];

    args.push("-c:a".to_owned());
    args.push(encoder_name(output));

    if let Some(kbps) = output.bitrate_kbps {
        args.push("-b:a".to_owned());
        args.push(format!("{kbps}k"));
    }

    args.push("-ar".to_owned());
    args.push(output.sample_rate_hz.to_string());

    if matches!(plan.action, Action::Encode { dither: true }) {
        // Triangular high-pass pushes the dither noise up out of the range the
        // ear is most sensitive to, which is why it is the usual choice for
        // material people listen to rather than measure.
        args.push("-af".to_owned());
        args.push("aresample=dither_method=triangular_hp".to_owned());
    }

    // One thread per encode: see the test for why the parallelism lives above.
    args.push("-threads".to_owned());
    args.push("1".to_owned());

    args.push("-nostats".to_owned());
    args.push("-progress".to_owned());
    args.push("pipe:1".to_owned());

    args
}

/// The ffmpeg encoder that writes this output.
///
/// PCM needs one per depth *and* byte order, since AIFF stores samples
/// big-endian where WAV stores them little-endian. Depth is unknown only when
/// the source never declared one, and 16-bit is the safe reading of that.
fn encoder_name(output: &AudioSpec) -> String {
    match output.codec {
        Codec::Mp3 => "libmp3lame".to_owned(),
        Codec::AacLc => "aac".to_owned(),
        Codec::Alac => "alac".to_owned(),
        Codec::Flac => "flac".to_owned(),
        Codec::PcmWav => format!("pcm_s{}le", output.bit_depth.unwrap_or(16)),
        Codec::PcmAiff => format!("pcm_s{}be", output.bit_depth.unwrap_or(16)),
    }
}

fn resolve(source: &AudioSpec, target: &Target) -> AudioSpec {
    let lossy = is_lossy(target.codec);

    AudioSpec {
        codec: target.codec,
        sample_rate_hz: match target.sample_rate {
            SampleRatePolicy::Preserve => source.sample_rate_hz,
            SampleRatePolicy::Fixed(hz) => hz,
            SampleRatePolicy::CapAt(hz) => source.sample_rate_hz.min(hz),
        },
        bit_depth: if lossy {
            None
        } else {
            match target.bit_depth {
                BitDepthPolicy::Preserve => source.bit_depth,
                BitDepthPolicy::Fixed(bits) => Some(bits),
                BitDepthPolicy::CapAt(bits) => source.bit_depth.map(|depth| depth.min(bits)),
            }
        },
        bitrate_kbps: if lossy { target.bitrate_kbps } else { None },
    }
}

/// Dither belongs to requantisation, not to resampling: it decorrelates the
/// error left behind when bits are thrown away, which is otherwise audible as
/// distortion rather than as noise. Changing the rate does not throw bits away,
/// and a lossy encoder shapes its own noise, so neither wants it.
fn shortens_word_length(source: &AudioSpec, output: &AudioSpec) -> bool {
    match (source.bit_depth, output.bit_depth) {
        (Some(from), Some(to)) => to < from,
        _ => false,
    }
}

const fn is_lossy(codec: Codec) -> bool {
    matches!(codec, Codec::Mp3 | Codec::AacLc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::AudioSpec;
    use crate::device::Codec;

    fn wav(sample_rate_hz: u32, bits: u8) -> AudioSpec {
        AudioSpec {
            codec: Codec::PcmWav,
            sample_rate_hz,
            bit_depth: Some(bits),
            bitrate_kbps: None,
        }
    }

    fn aiff_at(bits: u8) -> Target {
        Target {
            codec: Codec::PcmAiff,
            sample_rate: SampleRatePolicy::Preserve,
            bit_depth: BitDepthPolicy::Fixed(bits),
            bitrate_kbps: None,
        }
    }

    #[test]
    fn the_built_in_profiles_resolve_by_name() {
        assert_eq!(Target::by_name("cdj-safe"), Some(Target::CDJ_SAFE));
        assert_eq!(
            Target::by_name("lossless").map(|t| t.codec),
            Some(Codec::PcmAiff)
        );
        assert_eq!(
            Target::by_name("archive").map(|t| t.codec),
            Some(Codec::Flac)
        );
        assert_eq!(Target::by_name("nope"), None);
    }

    /// A built-in profile that quietly produces something a player rejects
    /// would be worse than having no profile at all. This holds the profiles
    /// and the device table to each other: change either and this fails.
    ///
    /// `archive` is left out on purpose. It preserves the source for storage,
    /// so it is the one profile not claiming to be booth-ready.
    #[test]
    fn the_booth_profiles_clear_every_player() {
        let hi_res = AudioSpec {
            codec: Codec::Flac,
            sample_rate_hz: 96_000,
            bit_depth: Some(24),
            bitrate_kbps: None,
        };

        for name in ["cdj-safe", "lossless"] {
            let target = Target::by_name(name).expect(name);
            let output = plan(&hi_res, &target).output;

            for device in crate::device::DEVICES {
                let issues = crate::compat::check(&output, device);
                assert!(
                    issues.is_empty(),
                    "{name} fails on {}: {issues:?}",
                    device.id
                );
            }
        }
    }

    /// Archive keeps what it was given, which is the point: it is for the copy
    /// you keep, not the one you play.
    #[test]
    fn archive_preserves_the_source() {
        let hi_res = AudioSpec {
            codec: Codec::Flac,
            sample_rate_hz: 96_000,
            bit_depth: Some(24),
            bitrate_kbps: None,
        };
        let output = plan(&hi_res, &Target::by_name("archive").expect("archive")).output;

        assert_eq!(output.sample_rate_hz, 96_000);
        assert_eq!(output.bit_depth, Some(24));
    }

    /// The fastest conversion is the one that does not happen. A library that
    /// is already in the target format should cost a file copy, not an encode.
    #[test]
    fn a_file_already_in_the_target_format_is_copied() {
        let source = AudioSpec {
            codec: Codec::Mp3,
            sample_rate_hz: 44_100,
            bit_depth: None,
            bitrate_kbps: Some(320),
        };
        let plan = plan(&source, &Target::CDJ_SAFE);

        assert_eq!(plan.action, Action::Copy);
        assert_eq!(plan.output, source);
    }

    #[test]
    fn hi_res_flac_becomes_the_default_mp3() {
        let source = AudioSpec {
            codec: Codec::Flac,
            sample_rate_hz: 96_000,
            bit_depth: Some(24),
            bitrate_kbps: None,
        };
        let plan = plan(&source, &Target::CDJ_SAFE);

        assert_eq!(plan.output.codec, Codec::Mp3);
        assert_eq!(plan.output.sample_rate_hz, 44_100);
        assert_eq!(plan.output.bitrate_kbps, Some(320));
        // A lossy encoder does its own noise shaping; dithering first would add
        // noise for nothing.
        assert_eq!(plan.action, Action::Encode { dither: false });
        // Bit depth stops being a property once the output is lossy.
        assert_eq!(plan.output.bit_depth, None);
    }

    /// Shortening the word length is the one case that needs dither. Without
    /// it, quantisation error correlates with the signal and is audible as
    /// distortion on fades and quiet passages.
    #[test]
    fn reducing_bit_depth_dithers() {
        let plan = plan(&wav(44_100, 32), &aiff_at(24));

        assert_eq!(plan.output.bit_depth, Some(24));
        assert_eq!(plan.action, Action::Encode { dither: true });
    }

    #[test]
    fn lengthening_bit_depth_does_not_dither() {
        let plan = plan(&wav(44_100, 16), &aiff_at(24));

        assert_eq!(plan.output.bit_depth, Some(24));
        assert_eq!(plan.action, Action::Encode { dither: false });
    }

    /// Resampling changes how often the signal is measured, not how finely.
    /// Dither belongs to the second, so a rate change alone must not trigger it.
    #[test]
    fn resampling_alone_does_not_dither() {
        let target = Target {
            codec: Codec::PcmWav,
            sample_rate: SampleRatePolicy::CapAt(48_000),
            bit_depth: BitDepthPolicy::Preserve,
            bitrate_kbps: None,
        };
        let plan = plan(&wav(96_000, 24), &target);

        assert_eq!(plan.output.sample_rate_hz, 48_000);
        assert_eq!(plan.output.bit_depth, Some(24));
        assert_eq!(plan.action, Action::Encode { dither: false });
    }

    fn pair(args: &[String], flag: &str) -> Option<String> {
        args.windows(2)
            .find(|window| window[0] == flag)
            .map(|window| window[1].clone())
    }

    /// AIFF stores samples big-endian and WAV little-endian, so the same depth
    /// needs a different encoder in each. Getting this backwards produces a
    /// file that plays as noise.
    #[test]
    fn pcm_encoders_follow_the_container_byte_order() {
        let to_aiff = plan(&wav(44_100, 24), &aiff_at(24));
        assert_eq!(
            pair(&encode_args(&to_aiff), "-c:a").as_deref(),
            Some("pcm_s24be")
        );

        let to_wav = plan(
            &wav(44_100, 24),
            &Target {
                codec: Codec::PcmWav,
                sample_rate: SampleRatePolicy::Preserve,
                bit_depth: BitDepthPolicy::Fixed(16),
                bitrate_kbps: None,
            },
        );
        assert_eq!(
            pair(&encode_args(&to_wav), "-c:a").as_deref(),
            Some("pcm_s16le")
        );
    }

    #[test]
    fn mp3_carries_its_encoder_bitrate_and_rate() {
        let source = AudioSpec {
            codec: Codec::Flac,
            sample_rate_hz: 96_000,
            bit_depth: Some(24),
            bitrate_kbps: None,
        };
        let args = encode_args(&plan(&source, &Target::CDJ_SAFE));

        assert_eq!(pair(&args, "-c:a").as_deref(), Some("libmp3lame"));
        assert_eq!(pair(&args, "-b:a").as_deref(), Some("320k"));
        assert_eq!(pair(&args, "-ar").as_deref(), Some("44100"));
    }

    #[test]
    fn dither_is_requested_only_when_the_word_shortens() {
        let shortening = encode_args(&plan(&wav(44_100, 32), &aiff_at(24)));
        assert!(
            shortening.iter().any(|arg| arg.contains("dither_method")),
            "got: {shortening:?}"
        );

        let lengthening = encode_args(&plan(&wav(44_100, 16), &aiff_at(24)));
        assert!(
            !lengthening.iter().any(|arg| arg.contains("dither")),
            "got: {lengthening:?}"
        );
    }

    /// Audio codecs get almost nothing from threading, so each encode stays on
    /// one thread and the parallelism lives a level up, one process per file.
    /// Left to itself ffmpeg would take every core for a single track.
    #[test]
    fn every_encode_is_single_threaded() {
        let args = encode_args(&plan(&wav(96_000, 24), &aiff_at(16)));
        assert_eq!(pair(&args, "-threads").as_deref(), Some("1"));
    }

    /// Progress has to be machine-readable: parsing ffmpeg's human output means
    /// re-parsing it every time the format shifts.
    #[test]
    fn progress_is_reported_on_stdout() {
        let args = encode_args(&plan(&wav(96_000, 24), &aiff_at(16)));
        assert_eq!(pair(&args, "-progress").as_deref(), Some("pipe:1"));
        assert!(args.iter().any(|arg| arg == "-nostats"));
    }

    /// A ceiling leaves anything already under it alone, so a 44.1 kHz track
    /// is not resampled up to meet it.
    #[test]
    fn a_ceiling_does_not_raise_a_lower_rate() {
        let target = Target {
            codec: Codec::PcmWav,
            sample_rate: SampleRatePolicy::CapAt(48_000),
            bit_depth: BitDepthPolicy::Preserve,
            bitrate_kbps: None,
        };
        let plan = plan(&wav(44_100, 24), &target);

        assert_eq!(plan.output.sample_rate_hz, 44_100);
        assert_eq!(plan.action, Action::Copy);
    }
}
