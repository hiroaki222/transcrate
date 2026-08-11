//! Runs plans through ffmpeg and reads the result back.
//!
//! The unit tests pin down which arguments get built. These check the thing
//! that actually matters: that ffmpeg accepts them, and that the file coming
//! out is the file the plan promised. An argument list can be perfectly
//! self-consistent and still produce something no CDJ will read.
//!
//! Skipped when ffmpeg or ffprobe is not on PATH.

use std::path::{Path, PathBuf};
use std::process::Command;

use transcrate_core::compat::AudioSpec;
use transcrate_core::device::Codec;
use transcrate_core::plan::{self, Action, BitDepthPolicy, SampleRatePolicy, Target};
use transcrate_core::{convert, probe};

fn tool_missing(tool: &str) -> bool {
    Command::new(tool).arg("-version").output().is_err()
}

fn workspace() -> PathBuf {
    let dir = std::env::temp_dir().join("transcrate-convert-real-files");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn encode_source(dir: &Path, name: &str, sample_rate_hz: u32, codec_args: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let source = format!("anullsrc=r={sample_rate_hz}:cl=stereo");

    let status = Command::new("ffmpeg")
        .args([
            "-v", "error", "-y", "-f", "lavfi", "-i", &source, "-t", "0.2",
        ])
        .args(codec_args)
        .arg(&path)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg failed to write {name}");

    path
}

/// Convert `input` by `plan` and read back what landed on disk.
fn convert_and_probe(plan: &plan::Plan, input: &Path, output: &Path) -> AudioSpec {
    convert::run(Path::new("ffmpeg"), plan, input, output).expect("convert");
    probe::run(Path::new("ffprobe"), output).expect("probe the output")
}

#[test]
fn the_output_matches_what_the_plan_promised() {
    if tool_missing("ffmpeg") || tool_missing("ffprobe") {
        eprintln!("skipping: ffmpeg or ffprobe is not on PATH");
        return;
    }

    let dir = workspace();
    let ffprobe = Path::new("ffprobe");

    // Hi-res FLAC down to the default profile: a new codec, a new rate, and a
    // bit depth that stops existing.
    let flac = encode_source(
        &dir,
        "hires.flac",
        96_000,
        &["-c:a", "flac", "-sample_fmt", "s32"],
    );
    let source = probe::run(ffprobe, &flac).expect("probe source");
    let to_mp3 = plan::plan(&source, &Target::CDJ_SAFE);

    let produced = convert_and_probe(&to_mp3, &flac, &dir.join("out.mp3"));
    assert_eq!(produced.codec, Codec::Mp3);
    assert_eq!(produced.sample_rate_hz, 44_100);
    assert_eq!(produced.bitrate_kbps, to_mp3.output.bitrate_kbps);
    assert_eq!(produced.bit_depth, to_mp3.output.bit_depth);
}

/// The case the dither exists for, and the one where the encoder name has to be
/// big-endian or the result is noise.
#[test]
fn a_dithered_reduction_into_aiff_lands_at_the_planned_depth() {
    if tool_missing("ffmpeg") || tool_missing("ffprobe") {
        eprintln!("skipping: ffmpeg or ffprobe is not on PATH");
        return;
    }

    let dir = workspace();
    let float32 = encode_source(&dir, "float32.wav", 48_000, &["-c:a", "pcm_f32le"]);
    let source = probe::run(Path::new("ffprobe"), &float32).expect("probe source");

    let to_aiff = plan::plan(
        &source,
        &Target {
            codec: Codec::PcmAiff,
            sample_rate: SampleRatePolicy::Preserve,
            bit_depth: BitDepthPolicy::Fixed(24),
            bitrate_kbps: None,
        },
    );
    assert_eq!(to_aiff.action, Action::Encode { dither: true });

    let produced = convert_and_probe(&to_aiff, &float32, &dir.join("out.aiff"));
    assert_eq!(produced.codec, Codec::PcmAiff);
    assert_eq!(produced.bit_depth, Some(24));
    assert_eq!(produced.sample_rate_hz, 48_000);
}

/// A copy has to leave the bytes alone. Re-encoding a file that already matches
/// would spend time to produce something slightly worse.
#[test]
fn a_copy_reproduces_the_source_byte_for_byte() {
    if tool_missing("ffmpeg") || tool_missing("ffprobe") {
        eprintln!("skipping: ffmpeg or ffprobe is not on PATH");
        return;
    }

    let dir = workspace();
    let source_path = encode_source(
        &dir,
        "already.mp3",
        44_100,
        &["-c:a", "libmp3lame", "-b:a", "320k"],
    );
    let source = probe::run(Path::new("ffprobe"), &source_path).expect("probe source");

    let copy = plan::plan(&source, &Target::CDJ_SAFE);
    assert_eq!(copy.action, Action::Copy);

    let output_path = dir.join("copied.mp3");
    convert::run(Path::new("ffmpeg"), &copy, &source_path, &output_path).expect("convert");

    let before = std::fs::read(&source_path).expect("read source");
    let after = std::fs::read(&output_path).expect("read output");
    assert_eq!(before, after, "a copy changed the file");
}
