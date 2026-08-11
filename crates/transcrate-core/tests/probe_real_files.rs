//! Round-trips real files through ffmpeg and ffprobe.
//!
//! The unit tests parse captured ffprobe output, which pins down the parsing
//! but not the assumption underneath it: that ffprobe still reports these
//! fields this way. These tests encode files and read them back, so a change in
//! ffmpeg's output shows up here rather than on someone's USB stick.

mod common;

use std::path::Path;

use common::{encode, tools_available, workspace};
use transcrate_core::device::Codec;
use transcrate_core::probe;

#[test]
fn reads_back_what_ffmpeg_wrote() {
    if !tools_available() {
        return;
    }

    let dir = workspace("probe-round-trip");
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
    if !tools_available() {
        return;
    }

    let dir = workspace("probe-not-audio");
    let path = dir.join("not-audio.txt");
    std::fs::write(&path, b"this is not a media file").expect("write file");

    assert!(probe::run(Path::new("ffprobe"), &path).is_err());
}
