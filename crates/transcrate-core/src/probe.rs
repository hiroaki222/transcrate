//! Reading what a file actually contains, as opposed to what it is named.

use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::compat::AudioSpec;
use crate::device::Codec;

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("could not run ffprobe: {source}")]
    NotRunnable {
        #[source]
        source: std::io::Error,
    },
    #[error("ffprobe could not read the file: {stderr}")]
    Rejected { stderr: String },
    #[error("ffprobe did not return valid JSON: {0}")]
    Malformed(#[from] serde_json::Error),
    #[error("the file contains no audio stream")]
    NoAudioStream,
    #[error("unsupported codec: {0}")]
    UnsupportedCodec(String),
    #[error("ffprobe reported an unreadable {field}: {value}")]
    UnreadableField { field: &'static str, value: String },
}

/// The invocation this module knows how to read.
///
/// ffprobe reports only what it is asked for, so this list and the fields the
/// parser reads have to stay in step.
const PROBE_ARGS: [&str; 8] = [
    "-v",
    "error",
    "-select_streams",
    "a:0",
    "-show_entries",
    "format=format_name:stream=codec_name,profile,sample_rate,bits_per_raw_sample,bits_per_sample,bit_rate",
    "-print_format",
    "json",
];

/// Read `file` using the ffprobe binary at `ffprobe`.
///
/// The binary is passed in rather than looked up here, so the caller decides
/// between a system installation and a bundled one.
///
/// # Errors
///
/// Fails when ffprobe cannot be started, rejects the file, or describes it in
/// terms this program does not handle.
pub fn run(ffprobe: &Path, file: &Path) -> Result<AudioSpec, ProbeError> {
    let output = Command::new(ffprobe)
        .args(PROBE_ARGS)
        .arg(file)
        .output()
        .map_err(|source| ProbeError::NotRunnable { source })?;

    if !output.status.success() {
        return Err(ProbeError::Rejected {
            stderr: why_it_failed(&output),
        });
    }

    parse(&String::from_utf8_lossy(&output.stdout))
}

/// What a failed run said, or what can be said about it when it said nothing.
///
/// A binary that exits non-zero without writing to stderr leaves the message
/// ending at the colon, which tells the reader only that something went wrong.
/// The status is the one fact left, and it is the one that separates "not a
/// media file" from "that path is not ffprobe at all".
fn why_it_failed(output: &std::process::Output) -> String {
    let said = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !said.is_empty() {
        return said;
    }

    format!("it wrote nothing and exited with {}", output.status)
}

/// The subset of `ffprobe -print_format json` that says what a stream is.
#[derive(Deserialize)]
struct ProbeOutput {
    streams: Vec<Stream>,
    format: Format,
}

#[derive(Deserialize)]
struct Stream {
    codec_name: String,
    /// Only present for codecs that have profiles, which among the ones read
    /// here means AAC alone.
    profile: Option<String>,
    /// Reported as a string, not a number.
    sample_rate: String,
    /// Present for formats that carry a declared depth, and the only field that
    /// tells the truth about one: FLAC at 24 bits is reported with a 32-bit
    /// sample format.
    bits_per_raw_sample: Option<String>,
    /// Zero for every compressed codec, lossless ones included.
    bits_per_sample: Option<u8>,
    /// Bits per second.
    bit_rate: Option<String>,
}

#[derive(Deserialize)]
struct Format {
    /// One name for most containers, a comma-separated list for the MP4 family.
    format_name: String,
}

/// Turn ffprobe's JSON into the same shape the compatibility checks take.
///
/// # Errors
///
/// Fails when the output is not the expected JSON, holds no audio stream, or
/// names a codec this program does not handle.
pub fn parse(probe_json: &str) -> Result<AudioSpec, ProbeError> {
    let output: ProbeOutput = serde_json::from_str(probe_json)?;
    let stream = output
        .streams
        .into_iter()
        .next()
        .ok_or(ProbeError::NoAudioStream)?;

    let codec = classify(
        &stream.codec_name,
        stream.profile.as_deref(),
        &output.format.format_name,
    )
    .ok_or_else(|| ProbeError::UnsupportedCodec(name_for_error(&stream)))?;

    let sample_rate_hz = stream
        .sample_rate
        .parse()
        .map_err(|_| ProbeError::UnreadableField {
            field: "sample_rate",
            value: stream.sample_rate.clone(),
        })?;

    // `bits_per_sample` is zero for anything compressed, so it only answers for
    // raw PCM — and there it is the only field present.
    let bit_depth = stream
        .bits_per_raw_sample
        .as_deref()
        .and_then(|bits| bits.parse().ok())
        .or_else(|| stream.bits_per_sample.filter(|bits| *bits > 0));

    // Lossless codecs report a bitrate too, but it describes how well the file
    // compressed rather than a setting any player checks.
    let bitrate_kbps = if matches!(codec, Codec::Mp3 | Codec::AacLc) {
        stream
            .bit_rate
            .as_deref()
            .and_then(|bps| bps.parse::<u32>().ok())
            .map(|bps| u16::try_from(bps / 1000).unwrap_or(u16::MAX))
    } else {
        None
    };

    Ok(AudioSpec {
        codec,
        sample_rate_hz,
        bit_depth,
        bitrate_kbps,
    })
}

/// PCM carries no hint of its container — `pcm_s16be` turns up in both WAV and
/// AIFF — so the container settles it. Everything else is named outright.
fn classify(codec_name: &str, profile: Option<&str>, format_name: &str) -> Option<Codec> {
    match codec_name {
        "mp3" => Some(Codec::Mp3),
        // Every manufacturer table here was written for AAC Low Complexity.
        // The other profiles arrive under the same codec name, so trusting the
        // name alone judges an HE-AAC file against limits belonging to a
        // format it is not, and passes it. A stream ffprobe declines to profile
        // is left unsupported for the same reason: not knowing is not a pass.
        "aac" => (profile == Some("LC")).then_some(Codec::AacLc),
        "alac" => Some(Codec::Alac),
        "flac" => Some(Codec::Flac),
        name if name.starts_with("pcm_") => {
            format_name
                .split(',')
                .find_map(|container| match container {
                    "wav" => Some(Codec::PcmWav),
                    "aiff" => Some(Codec::PcmAiff),
                    _ => None,
                })
        }
        _ => None,
    }
}

/// What to call a stream nothing here can handle. For AAC the codec name on its
/// own would read as a contradiction — the name is supported and the profile is
/// what was refused — so the profile is named alongside it.
fn name_for_error(stream: &Stream) -> String {
    match &stream.profile {
        Some(profile) => format!("{} ({profile})", stream.codec_name),
        None => stream.codec_name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::Codec;

    // Every fixture below is verbatim output from
    //
    //   ffprobe -v error -select_streams a:0 \
    //     -show_entries format=format_name:stream=codec_name,profile,\
    //   sample_rate,bits_per_raw_sample,bits_per_sample,bit_rate \
    //     -print_format json FILE
    //
    // captured from files ffmpeg 8.1.1 produced. They are not hand-written:
    // the awkward parts of this format are exactly the parts worth testing.

    const FLAC_96K_24BIT: &str = r#"{"programs":[],"stream_groups":[],"streams":[{"codec_name":"flac","sample_rate":"96000","bits_per_sample":0,"bits_per_raw_sample":"24"}],"format":{"format_name":"flac"}}"#;

    const ALAC_IN_M4A: &str = r#"{"programs":[],"stream_groups":[],"streams":[{"codec_name":"alac","sample_rate":"44100","bits_per_sample":0,"bit_rate":"5440","bits_per_raw_sample":"16"}],"format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2"}}"#;

    const AAC_IN_M4A: &str = r#"{"programs":[],"stream_groups":[],"streams":[{"codec_name":"aac","profile":"LC","sample_rate":"44100","bits_per_sample":0,"bit_rate":"121319"}],"format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2"}}"#;

    /// The same encoder, the same container, the same codec name, and a format
    /// no manufacturer table here covers.
    const HE_AAC_IN_M4A: &str = r#"{"programs":[],"stream_groups":[],"streams":[{"codec_name":"aac","profile":"HE-AAC","sample_rate":"44100","bits_per_sample":0,"bit_rate":"39804"}],"format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2"}}"#;

    const WAV_FLOAT32_48K: &str = r#"{"programs":[],"stream_groups":[],"streams":[{"codec_name":"pcm_f32le","sample_rate":"48000","bits_per_sample":32,"bit_rate":"3072000"}],"format":{"format_name":"wav"}}"#;

    const AIFF_16BIT_44K: &str = r#"{"programs":[],"stream_groups":[],"streams":[{"codec_name":"pcm_s16be","sample_rate":"44100","bits_per_sample":16,"bit_rate":"1411200"}],"format":{"format_name":"aiff"}}"#;

    const MP3_CBR_320: &str = r#"{"programs":[],"stream_groups":[],"streams":[{"codec_name":"mp3","sample_rate":"44100","bits_per_sample":0,"bit_rate":"320000"}],"format":{"format_name":"mp3"}}"#;

    /// ffprobe reports this file as `sample_fmt: s32` while its samples are
    /// 24-bit. Trusting the sample format would call a CDJ-playable file
    /// unplayable, so the depth has to come from `bits_per_raw_sample`.
    #[test]
    fn bit_depth_comes_from_the_raw_sample_field() {
        let spec = parse(FLAC_96K_24BIT).expect("parse flac");
        assert_eq!(spec.codec, Codec::Flac);
        assert_eq!(spec.sample_rate_hz, 96_000);
        assert_eq!(spec.bit_depth, Some(24));
    }

    /// The reason the whole table keys on codecs: these two files share an
    /// extension and a container, and several players take only one of them.
    #[test]
    fn alac_and_aac_are_told_apart_inside_the_same_container() {
        assert_eq!(parse(ALAC_IN_M4A).expect("parse alac").codec, Codec::Alac);
        assert_eq!(parse(AAC_IN_M4A).expect("parse aac").codec, Codec::AacLc);
    }

    /// Both files say `aac`, and only one of them is the format the
    /// compatibility tables were written against. Reading the name alone put an
    /// HE-AAC file in front of the AAC-LC limits, where its low bitrate and
    /// 44.1 kHz cleared every one of them and it was reported as playing
    /// everywhere.
    #[test]
    fn only_the_low_complexity_aac_profile_is_taken_as_aac() {
        assert_eq!(parse(AAC_IN_M4A).expect("parse aac lc").codec, Codec::AacLc);

        let refused = parse(HE_AAC_IN_M4A).expect_err("he-aac must not parse as aac-lc");
        assert!(
            matches!(&refused, ProbeError::UnsupportedCodec(name) if name.contains("HE-AAC")),
            "the profile has to be named, or the message reads as a contradiction: {refused}"
        );
    }

    /// A file dragged out of a DAW. No player accepts 32-bit, so the depth has
    /// to survive parsing rather than being rounded to something plausible.
    #[test]
    fn float32_keeps_its_real_depth() {
        let spec = parse(WAV_FLOAT32_48K).expect("parse float wav");
        assert_eq!(spec.codec, Codec::PcmWav);
        assert_eq!(spec.bit_depth, Some(32));
    }

    /// PCM says nothing about its container: `pcm_s16be` appears in both, so
    /// WAV and AIFF are separated by the format rather than the codec.
    #[test]
    fn pcm_is_assigned_to_its_container() {
        assert_eq!(
            parse(AIFF_16BIT_44K).expect("parse aiff").codec,
            Codec::PcmAiff
        );
        assert_eq!(
            parse(WAV_FLOAT32_48K).expect("parse wav").codec,
            Codec::PcmWav
        );
    }

    /// The parser reads seven fields, and ffprobe only reports what it is asked
    /// for. Dropping one from the request would leave the parser reading a
    /// field that is never there, which fails as a missing value rather than as
    /// anything that points at this list.
    #[test]
    fn the_request_covers_every_field_the_parser_reads() {
        let rendered = PROBE_ARGS.join(" ");
        for field in [
            "codec_name",
            "profile",
            "sample_rate",
            "bits_per_raw_sample",
            "bits_per_sample",
            "bit_rate",
            "format_name",
        ] {
            assert!(rendered.contains(field), "PROBE_ARGS omits {field}");
        }
    }

    /// Lossy files have a bitrate and no meaningful depth; ffprobe reports the
    /// rate in bits per second.
    #[test]
    fn lossy_reports_kbps_and_no_depth() {
        let spec = parse(MP3_CBR_320).expect("parse mp3");
        assert_eq!(spec.codec, Codec::Mp3);
        assert_eq!(spec.bitrate_kbps, Some(320));
        assert_eq!(spec.bit_depth, None);
    }
}
