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

/// What happens to embedded artwork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Artwork {
    /// Carry it across untouched, labelling the stream the way players expect.
    Keep,
    /// Leave it behind.
    Remove,
}

/// What happens to the tags a file arrives with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MetadataPolicy {
    /// Tag fields to empty out. Everything else is carried across.
    pub clear: &'static [&'static str],
    pub artwork: Artwork,
}

impl MetadataPolicy {
    /// Comment and lyrics go, everything else stays.
    ///
    /// Those two are where shops and rippers leave their advertising, and a CDJ
    /// puts the comment in the browser right next to the title. The rest —
    /// title, artist, album, genre, key, BPM — is what the browser is for.
    pub const DJ: Self = Self {
        clear: &["comment", "lyrics-eng"],
        artwork: Artwork::Keep,
    };

    /// The same, for people who keep their own cue notes or a Camelot key in
    /// the comment. The lyrics still go: nobody reads those off a CDJ, and
    /// they are the other thing shops fill in.
    pub const KEEPING_COMMENTS: Self = Self {
        clear: &["lyrics-eng"],
        artwork: Artwork::Keep,
    };

    /// Whether applying this would change a file at all.
    ///
    /// Keeping the artwork and clearing nothing leaves the bytes as they were,
    /// which is what makes a plain copy correct.
    fn rewrites_anything(&self) -> bool {
        !self.clear.is_empty() || self.artwork == Artwork::Remove
    }
}

/// What to convert into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Target {
    pub codec: Codec,
    pub sample_rate: SampleRatePolicy,
    /// Ignored for lossy codecs, which carry no bit depth.
    pub bit_depth: BitDepthPolicy,
    /// Ignored for lossless codecs.
    pub bitrate_kbps: Option<u16>,
    pub metadata: MetadataPolicy,
}

impl Target {
    /// Plays on every player in the table: MP3 at 320 kbps and 44.1 kHz.
    pub const CDJ_SAFE: Self = Self {
        codec: Codec::Mp3,
        sample_rate: SampleRatePolicy::Fixed(44_100),
        bit_depth: BitDepthPolicy::Preserve,
        bitrate_kbps: Some(320),
        metadata: MetadataPolicy::DJ,
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
        metadata: MetadataPolicy::DJ,
    };

    /// For the copy you keep rather than the one you play: FLAC at whatever the
    /// source was. Makes no promise about any player.
    pub const ARCHIVE: Self = Self {
        codec: Codec::Flac,
        sample_rate: SampleRatePolicy::Preserve,
        bit_depth: BitDepthPolicy::Preserve,
        bitrate_kbps: None,
        metadata: MetadataPolicy::DJ,
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

    /// Format names accepted where a container is named directly.
    pub const FORMATS: [&'static str; 6] = ["mp3", "aac", "alac", "flac", "wav", "aiff"];

    /// A target that changes the format and nothing else.
    ///
    /// Where a profile carries limits with it, this keeps the source's rate and
    /// depth. Naming a format is for "put this in AIFF", not "make this safe",
    /// and the two want different answers.
    pub fn from_format(format: &str) -> Option<Self> {
        let codec = match format {
            "mp3" => Codec::Mp3,
            "aac" => Codec::AacLc,
            "alac" => Codec::Alac,
            "flac" => Codec::Flac,
            "wav" => Codec::PcmWav,
            "aiff" => Codec::PcmAiff,
            _ => return None,
        };

        Some(Self {
            codec,
            sample_rate: SampleRatePolicy::Preserve,
            bit_depth: BitDepthPolicy::Preserve,
            // A lossy codec left without one encodes at ffmpeg's default, which
            // is far below anything worth playing out.
            bitrate_kbps: is_lossy(codec).then_some(320),
            metadata: MetadataPolicy::DJ,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "action")]
pub enum Action {
    /// The source already matches the target, so copy the bytes across. This is
    /// the cheapest outcome and worth reaching for: a library that is already
    /// in the right format should not be re-encoded to prove it.
    Copy,
    /// The audio is already what was asked for, but the tags are not. The
    /// stream is copied across untouched and only the metadata is rewritten,
    /// so a lossy source loses nothing to a change of text.
    Retag,
    Encode {
        dither: bool,
    },
}

/// What one file's conversion will produce, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Plan {
    pub output: AudioSpec,
    pub action: Action,
    pub metadata: MetadataPolicy,
}

/// Work out the output and how to reach it.
pub fn plan(source: &AudioSpec, target: &Target) -> Plan {
    let output = resolve(source, target);

    let action = if output != *source {
        Action::Encode {
            dither: shortens_word_length(source, &output),
        }
    } else if target.metadata.rewrites_anything() {
        Action::Retag
    } else {
        Action::Copy
    };

    Plan {
        output,
        action,
        metadata: target.metadata,
    }
}

/// The ffmpeg options that carry out an encode, to be placed after `-i INPUT`
/// and before the output path.
///
/// Meaningless for a [`Action::Copy`] plan, which is a file copy rather than an
/// ffmpeg run.
pub fn encode_args(plan: &Plan) -> Vec<String> {
    let output = &plan.output;
    let mut args = vec!["-map".to_owned(), "0:a:0".to_owned()];

    if plan.metadata.artwork == Artwork::Keep {
        // The `?` is what lets one set of arguments suit a file with artwork
        // and a file without: without it, a track with no sleeve is an error.
        args.push("-map".to_owned());
        args.push("0:v?".to_owned());
    }

    args.push("-c:a".to_owned());

    // A retag leaves the audio exactly as it arrived: re-encoding it to change
    // a string would cost quality on a lossy source and time on any other.
    // Bitrate, rate and dither all describe an encode that is not happening.
    if plan.action == Action::Retag {
        args.push("copy".to_owned());
        args.extend(metadata_args(plan.metadata, output.codec));
        args.push("-nostats".to_owned());
        args.push("-progress".to_owned());
        args.push("pipe:1".to_owned());
        return args;
    }

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

    args.extend(metadata_args(plan.metadata, output.codec));

    // One thread per encode: see the test for why the parallelism lives above.
    args.push("-threads".to_owned());
    args.push("1".to_owned());

    args.push("-nostats".to_owned());
    args.push("-progress".to_owned());
    args.push("pipe:1".to_owned());

    args
}

/// Tag handling: which fields to empty, what to do with the artwork, and which
/// ID3 version to write where that means anything.
fn metadata_args(policy: MetadataPolicy, codec: Codec) -> Vec<String> {
    // Everything the source carried comes across; the clears below then remove
    // what was asked for.
    let mut args = vec!["-map_metadata".to_owned(), "0".to_owned()];

    for field in policy.clear {
        args.push("-metadata".to_owned());
        args.push(format!("{field}="));
    }

    if policy.artwork == Artwork::Keep {
        // Copied rather than re-encoded: nothing here improves a JPEG.
        args.push("-c:v".to_owned());
        args.push("copy".to_owned());
        // Without the disposition a player treats the picture as a video track
        // rather than a sleeve, and these two stream tags are what rekordbox
        // and the CDJ browser read to place it.
        args.push("-disposition:v".to_owned());
        args.push("attached_pic".to_owned());
        args.push("-metadata:s:v".to_owned());
        args.push("title=Album cover".to_owned());
        args.push("-metadata:s:v".to_owned());
        args.push("comment=Cover (front)".to_owned());
    }

    match codec {
        // The AIFF muxer writes no ID3 chunk unless asked, and the artwork goes
        // with it. AIFF's own chunks still carry the title and artist, so the
        // loss shows up as a missing sleeve rather than as an untagged file.
        Codec::PcmAiff => {
            args.push("-write_id3v2".to_owned());
            args.push("1".to_owned());
        }
        Codec::Mp3 => {}
        // FLAC has Vorbis comments, M4A has iTunes atoms, WAV has neither worth
        // relying on. An ID3 flag would be noise.
        _ => return args,
    }

    // ffmpeg writes 2.4 unless told otherwise, and players are more consistent
    // with 2.3.
    args.push("-id3v2_version".to_owned());
    args.push("3".to_owned());

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
            metadata: MetadataPolicy::DJ,
        }
    }

    /// Naming a format alone means "this container, nothing else changed".
    /// A profile carries limits with it; this does not, which is the point of
    /// having both.
    #[test]
    fn a_format_alone_keeps_the_source_rate_and_depth() {
        let hi_res = AudioSpec {
            codec: Codec::Flac,
            sample_rate_hz: 96_000,
            bit_depth: Some(24),
            bitrate_kbps: None,
        };
        let output = plan(&hi_res, &Target::from_format("aiff").expect("aiff")).output;

        assert_eq!(output.codec, Codec::PcmAiff);
        assert_eq!(output.sample_rate_hz, 96_000);
        assert_eq!(output.bit_depth, Some(24));
    }

    /// A lossy format has to arrive with a bitrate or ffmpeg picks its own,
    /// which is far below anything worth playing out.
    #[test]
    fn a_lossy_format_arrives_with_a_bitrate() {
        assert_eq!(
            Target::from_format("mp3").and_then(|t| t.bitrate_kbps),
            Some(320)
        );
        assert_eq!(
            Target::from_format("aac").and_then(|t| t.bitrate_kbps),
            Some(320)
        );
        assert_eq!(
            Target::from_format("flac").and_then(|t| t.bitrate_kbps),
            None
        );
    }

    #[test]
    fn every_listed_format_resolves() {
        for name in Target::FORMATS {
            assert!(
                Target::from_format(name).is_some(),
                "{name} is listed but unknown"
            );
        }
        assert_eq!(Target::from_format("ogg"), None);
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

    fn already_cdj_safe() -> AudioSpec {
        AudioSpec {
            codec: Codec::Mp3,
            sample_rate_hz: 44_100,
            bit_depth: None,
            bitrate_kbps: Some(320),
        }
    }

    const UNTOUCHED: MetadataPolicy = MetadataPolicy {
        clear: &[],
        artwork: Artwork::Keep,
    };

    /// The fastest conversion is the one that does not happen. A library that
    /// is already in the target format, with nothing to rewrite, should cost a
    /// file copy and no more.
    #[test]
    fn a_file_that_needs_nothing_is_copied() {
        let target = Target {
            metadata: UNTOUCHED,
            ..Target::CDJ_SAFE
        };
        let plan = plan(&already_cdj_safe(), &target);

        assert_eq!(plan.action, Action::Copy);
        assert_eq!(plan.output, already_cdj_safe());
    }

    /// Tags to clear on a file that is otherwise already right has to rewrite
    /// the file — a copy carries the tag across untouched, which is the whole
    /// problem. The audio is stream-copied, so nothing is re-encoded to change
    /// a string, and a lossy source loses nothing.
    #[test]
    fn a_file_needing_only_tag_changes_is_stream_copied() {
        let plan = plan(&already_cdj_safe(), &Target::CDJ_SAFE);
        assert_eq!(plan.action, Action::Retag);

        let args = encode_args(&plan);
        assert!(
            pairs_contain(&args, "-c:a", "copy"),
            "audio should not be re-encoded to change a tag: {args:?}"
        );
        assert!(pairs_contain(&args, "-metadata", "comment="), "{args:?}");
        // Re-encoding options make no sense against a stream copy.
        assert!(!args.iter().any(|arg| arg == "-b:a"), "{args:?}");
        assert!(!args.iter().any(|arg| arg == "-af"), "{args:?}");
    }

    /// Removing artwork is a rewrite too, even with no tag fields to clear.
    #[test]
    fn dropping_artwork_alone_is_enough_to_rewrite() {
        let target = Target {
            metadata: MetadataPolicy {
                clear: &[],
                artwork: Artwork::Remove,
            },
            ..Target::CDJ_SAFE
        };

        assert_eq!(plan(&already_cdj_safe(), &target).action, Action::Retag);
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
            metadata: MetadataPolicy::DJ,
        };
        let plan = plan(&wav(96_000, 24), &target);

        assert_eq!(plan.output.sample_rate_hz, 48_000);
        assert_eq!(plan.output.bit_depth, Some(24));
        assert_eq!(plan.action, Action::Encode { dither: false });
    }

    fn pairs_contain(args: &[String], flag: &str, value: &str) -> bool {
        args.windows(2)
            .any(|pair| pair[0] == flag && pair[1] == value)
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
                metadata: MetadataPolicy::DJ,
            },
        );
        assert_eq!(
            pair(&encode_args(&to_wav), "-c:a").as_deref(),
            Some("pcm_s16le")
        );
    }

    fn flac_source() -> AudioSpec {
        AudioSpec {
            codec: Codec::Flac,
            sample_rate_hz: 96_000,
            bit_depth: Some(24),
            bitrate_kbps: None,
        }
    }

    /// Artwork rides along. A track with no sleeve looks broken in a player's
    /// browser, and dropping it silently is worse than not carrying it at all.
    ///
    /// `0:v?` is what makes this work without asking first: the `?` means "if
    /// there is one", so the same arguments suit a file with artwork and a file
    /// without.
    #[test]
    fn artwork_is_carried_across_with_its_stream_tags_set() {
        let args = encode_args(&plan(&flac_source(), &Target::CDJ_SAFE));

        assert!(pairs_contain(&args, "-map", "0:v?"), "{args:?}");
        assert!(pairs_contain(&args, "-c:v", "copy"), "{args:?}");
        assert!(
            args.iter().any(|arg| arg.contains("Album cover")),
            "artwork stream is not labelled: {args:?}"
        );
    }

    /// Plenty of people keep their own cue notes or a Camelot key in the
    /// comment, so emptying it has to be a choice rather than a rule. The
    /// lyrics go either way — nobody reads those off a CDJ.
    #[test]
    fn the_comment_can_be_kept() {
        let kept = MetadataPolicy::KEEPING_COMMENTS;
        assert!(!kept.clear.contains(&"comment"), "{:?}", kept.clear);
        assert!(kept.clear.contains(&"lyrics-eng"), "{:?}", kept.clear);

        let target = Target {
            metadata: kept,
            ..Target::CDJ_SAFE
        };
        let args = encode_args(&plan(&flac_source(), &target));

        assert!(!pairs_contain(&args, "-metadata", "comment="), "{args:?}");
        assert!(pairs_contain(&args, "-metadata", "lyrics-eng="), "{args:?}");
    }

    /// Shops and rippers leave their advertising in the comment, and a CDJ
    /// shows it in the browser next to the title.
    #[test]
    fn the_listed_tag_fields_are_emptied() {
        let args = encode_args(&plan(&flac_source(), &Target::CDJ_SAFE));

        assert!(pairs_contain(&args, "-metadata", "comment="), "{args:?}");
        assert!(pairs_contain(&args, "-metadata", "lyrics-eng="), "{args:?}");
    }

    /// ffmpeg writes ID3v2.4 unless told otherwise, and players are more
    /// consistent with 2.3.
    #[test]
    fn mp3_is_tagged_as_id3v2_3() {
        let args = encode_args(&plan(&flac_source(), &Target::CDJ_SAFE));
        assert!(pairs_contain(&args, "-id3v2_version", "3"), "{args:?}");
    }

    /// The AIFF muxer writes no ID3 at all unless asked, which takes the
    /// artwork down with it — the tags survive in AIFF's own chunks, so the
    /// loss is easy to miss.
    #[test]
    fn aiff_is_told_to_write_id3_in_the_first_place() {
        let args = encode_args(&plan(&flac_source(), &Target::LOSSLESS));

        assert!(pairs_contain(&args, "-write_id3v2", "1"), "{args:?}");
        assert!(pairs_contain(&args, "-id3v2_version", "3"), "{args:?}");
    }

    /// FLAC carries Vorbis comments, so an ID3 flag there is noise at best.
    #[test]
    fn formats_without_id3_are_not_told_about_it() {
        let args = encode_args(&plan(&flac_source(), &Target::ARCHIVE));
        assert!(!args.iter().any(|arg| arg.contains("id3")), "{args:?}");
    }

    /// Dropping artwork means not mapping the stream. Leaving it mapped and
    /// merely untagged would still carry the picture.
    #[test]
    fn removing_artwork_leaves_the_stream_unmapped() {
        let target = Target {
            metadata: MetadataPolicy {
                artwork: Artwork::Remove,
                ..Target::CDJ_SAFE.metadata
            },
            ..Target::CDJ_SAFE
        };
        let args = encode_args(&plan(&flac_source(), &target));

        assert!(!args.iter().any(|arg| arg == "0:v?"), "{args:?}");
        assert!(
            !args.iter().any(|arg| arg.contains("Album cover")),
            "{args:?}"
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
            // Nothing to rewrite, so a file already under the ceiling stays a
            // plain copy.
            metadata: UNTOUCHED,
        };
        let plan = plan(&wav(44_100, 24), &target);

        assert_eq!(plan.output.sample_rate_hz, 44_100);
        assert_eq!(plan.action, Action::Copy);
    }
}
