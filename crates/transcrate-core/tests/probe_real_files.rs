//! Round-trips real files through ffmpeg and ffprobe.
//!
//! The unit tests parse captured ffprobe output, which pins down the parsing
//! but not the assumption underneath it: that ffprobe still reports these
//! fields this way. These tests encode files and read them back, so a change in
//! ffmpeg's output shows up here rather than in someone's USB stick.
//!
//! Skipped when ffmpeg or ffprobe is not on PATH, so a checkout without them
//! still passes the rest of the suite.

use std::path::{Path, PathBuf};
use std::process::Command;

use transcrate_core::device::Codec;
use transcrate_core::probe;

fn tool_missing(tool: &str) -> bool {
    Command::new(tool).arg("-version").output().is_err()
}

/// Encode 0.1 s of silence with the given codec settings.
fn encode(dir: &Path, name: &str, sample_rate_hz: u32, codec_args: &[&str]) -> PathBuf {
    let path = dir.join(name);
    let source = format!("anullsrc=r={sample_rate_hz}:cl=stereo");

    let status = Command::new("ffmpeg")
        .args([
            "-v", "error", "-y", "-f", "lavfi", "-i", &source, "-t", "0.1",
        ])
        .args(codec_args)
        .arg(&path)
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg failed to write {name}");

    path
}

#[test]
fn reads_back_what_ffmpeg_wrote() {
    if tool_missing("ffmpeg") || tool_missing("ffprobe") {
        eprintln!("skipping: ffmpeg or ffprobe is not on PATH");
        return;
    }

    let dir = std::env::temp_dir().join("transcrate-probe-real-files");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let ffprobe = Path::new("ffprobe");

    // 24-bit FLAC, which ffprobe describes with a 32-bit sample format. The
    // depth has to come back as 24 or a playable file looks unplayable.
    let flac = encode(
        &dir,
        "hires.flac",
        96_000,
        &["-c:a", "flac", "-sample_fmt", "s32"],
    );
    let spec = probe::run(ffprobe, &flac).expect("probe flac");
    assert_eq!(spec.codec, Codec::Flac);
    assert_eq!(spec.sample_rate_hz, 96_000);
    assert_eq!(spec.bit_depth, Some(24));

    // The two codecs that share the .m4a extension.
    let alac = encode(&dir, "lossless.m4a", 44_100, &["-c:a", "alac"]);
    assert_eq!(
        probe::run(ffprobe, &alac).expect("probe alac").codec,
        Codec::Alac
    );

    let aac = encode(&dir, "lossy.m4a", 44_100, &["-c:a", "aac", "-b:a", "320k"]);
    assert_eq!(
        probe::run(ffprobe, &aac).expect("probe aac").codec,
        Codec::AacLc
    );

    // PCM in each container, told apart by the container alone.
    let wav = encode(&dir, "cd.wav", 44_100, &["-c:a", "pcm_s16le"]);
    let wav_spec = probe::run(ffprobe, &wav).expect("probe wav");
    assert_eq!(wav_spec.codec, Codec::PcmWav);
    assert_eq!(wav_spec.bit_depth, Some(16));

    let aiff = encode(&dir, "cd.aiff", 44_100, &["-c:a", "pcm_s16be"]);
    assert_eq!(
        probe::run(ffprobe, &aiff).expect("probe aiff").codec,
        Codec::PcmAiff
    );

    // A file straight out of a DAW: no player accepts 32-bit.
    let float32 = encode(&dir, "float32.wav", 48_000, &["-c:a", "pcm_f32le"]);
    assert_eq!(
        probe::run(ffprobe, &float32)
            .expect("probe float")
            .bit_depth,
        Some(32)
    );

    let mp3 = encode(
        &dir,
        "cbr320.mp3",
        44_100,
        &["-c:a", "libmp3lame", "-b:a", "320k"],
    );
    let mp3_spec = probe::run(ffprobe, &mp3).expect("probe mp3");
    assert_eq!(mp3_spec.codec, Codec::Mp3);
    assert_eq!(mp3_spec.bitrate_kbps, Some(320));
    assert_eq!(mp3_spec.bit_depth, None);
}

#[test]
fn a_file_that_is_not_audio_fails_rather_than_guessing() {
    if tool_missing("ffprobe") {
        eprintln!("skipping: ffprobe is not on PATH");
        return;
    }

    let dir = std::env::temp_dir().join("transcrate-probe-real-files");
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let path = dir.join("not-audio.txt");
    std::fs::write(&path, b"this is not a media file").expect("write file");

    assert!(probe::run(Path::new("ffprobe"), &path).is_err());
}
