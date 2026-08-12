//! Runs plans through ffmpeg and reads the result back.
//!
//! The unit tests pin down which arguments get built. These check the thing
//! that actually matters: that ffmpeg accepts them, and that the file coming
//! out is the file the plan promised. An argument list can be perfectly
//! self-consistent and still produce something no CDJ will read.

mod common;

use std::path::Path;

use common::{encode, tools_available, workspace};
use transcrate_core::compat::AudioSpec;
use transcrate_core::device::Codec;
use transcrate_core::plan::{
    self, Action, Artwork, BitDepthPolicy, MetadataPolicy, SampleRatePolicy, Target,
};
use transcrate_core::{convert, files, probe};

/// Convert `input` by `plan` and read back what landed on disk.
fn convert_and_probe(plan: &plan::Plan, input: &Path, output: &Path) -> AudioSpec {
    convert::run(Path::new("ffmpeg"), plan, input, output).expect("convert");
    probe::run(Path::new("ffprobe"), output).expect("probe the output")
}

#[test]
fn the_output_matches_what_the_plan_promised() {
    if !tools_available() {
        return;
    }

    let dir = workspace("convert-plan-promise");
    let ffprobe = Path::new("ffprobe");

    // Hi-res FLAC down to the default profile: a new codec, a new rate, and a
    // bit depth that stops existing.
    let flac = encode(
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

/// Working out what would happen is the first thing a window does with a folder
/// dropped on it, and someone who has not pressed convert yet should not find
/// their library seeded with empty `_transcrate` folders.
#[test]
fn working_out_a_job_writes_nothing_to_disk() {
    if !tools_available() {
        return;
    }

    let dir = workspace("prepare-is-read-only");
    let flac = encode(&dir, "track.flac", 44_100, &["-c:a", "flac"]);
    let _ = std::fs::remove_dir_all(dir.join(files::OUTPUT_FOLDER));

    let job = convert::prepare(&flac, None, Path::new("ffprobe"), &|_| Target::CDJ_SAFE)
        .expect("prepare");

    assert_eq!(job.output, dir.join(files::OUTPUT_FOLDER).join("track.mp3"));
    assert!(
        !dir.join(files::OUTPUT_FOLDER).exists(),
        "preparing a job created the output folder"
    );
}

/// Which leaves the run to make the folder, and it has to: every conversion
/// lands somewhere that did not exist a moment ago.
#[test]
fn a_run_creates_the_folder_it_writes_into() {
    if !tools_available() {
        return;
    }

    let dir = workspace("run-creates-its-folder");
    let flac = encode(&dir, "track.flac", 44_100, &["-c:a", "flac"]);

    let missing = dir.join("not-there-yet");
    let _ = std::fs::remove_dir_all(&missing);

    let source = probe::run(Path::new("ffprobe"), &flac).expect("probe source");
    let to_mp3 = plan::plan(&source, &Target::CDJ_SAFE);
    let output = missing.join("track.mp3");

    convert::run(Path::new("ffmpeg"), &to_mp3, &flac, &output).expect("convert");
    assert!(output.exists(), "the output was not written");
}

/// The case the dither exists for, and the one where the encoder name has to be
/// big-endian or the result is noise.
#[test]
fn a_dithered_reduction_into_aiff_lands_at_the_planned_depth() {
    if !tools_available() {
        return;
    }

    let dir = workspace("convert-dither");
    let float32 = encode(&dir, "float32.wav", 48_000, &["-c:a", "pcm_f32le"]);
    let source = probe::run(Path::new("ffprobe"), &float32).expect("probe source");

    let to_aiff = plan::plan(
        &source,
        &Target {
            codec: Codec::PcmAiff,
            sample_rate: SampleRatePolicy::Preserve,
            bit_depth: BitDepthPolicy::Fixed(24),
            bitrate_kbps: None,
            metadata: MetadataPolicy::DJ,
        },
    );
    assert_eq!(to_aiff.action, Action::Encode { dither: true });

    let produced = convert_and_probe(&to_aiff, &float32, &dir.join("out.aiff"));
    assert_eq!(produced.codec, Codec::PcmAiff);
    assert_eq!(produced.bit_depth, Some(24));
    assert_eq!(produced.sample_rate_hz, 48_000);
}

/// Results have to line up with the jobs that produced them. Running in
/// parallel means they finish out of order, and a failure landing at the wrong
/// index would blame the wrong file.
#[test]
fn results_keep_their_order_and_a_failure_stays_at_its_index() {
    if !tools_available() {
        return;
    }

    let dir = workspace("convert-parallel");
    let source_path = encode(&dir, "source.wav", 44_100, &["-c:a", "pcm_s16le"]);
    let source = probe::run(Path::new("ffprobe"), &source_path).expect("probe source");
    let to_aiff = plan::plan(&source, &Target::from_format("aiff").expect("aiff"));

    let job = |name: &str, input: &Path| convert::Job {
        plan: to_aiff,
        input: input.to_path_buf(),
        output: dir.join(name),
    };

    let missing = dir.join("does-not-exist.wav");
    let jobs = vec![
        job("first.aiff", &source_path),
        job("second.aiff", &missing),
        job("third.aiff", &source_path),
    ];

    let results = convert::run_all(Path::new("ffmpeg"), &jobs, 4, &|_, _| {});

    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok(), "{:?}", results[0]);
    assert!(results[1].is_err(), "a missing input should fail");
    assert!(results[2].is_ok(), "{:?}", results[2]);
}

/// Every job has to run, however many workers there are and however few jobs.
#[test]
fn all_jobs_run_whatever_the_concurrency() {
    if !tools_available() {
        return;
    }

    let dir = workspace("convert-parallel-all");
    let source_path = encode(&dir, "source.wav", 44_100, &["-c:a", "pcm_s16le"]);
    let source = probe::run(Path::new("ffprobe"), &source_path).expect("probe source");
    let to_aiff = plan::plan(&source, &Target::from_format("aiff").expect("aiff"));

    for concurrency in [1, 3, 16] {
        let jobs: Vec<_> = (0..5)
            .map(|index| convert::Job {
                plan: to_aiff,
                input: source_path.clone(),
                output: dir.join(format!("out-{concurrency}-{index}.aiff")),
            })
            .collect();

        let results = convert::run_all(Path::new("ffmpeg"), &jobs, concurrency, &|_, _| {});

        assert_eq!(results.len(), 5);
        assert!(
            results.iter().all(Result::is_ok),
            "concurrency {concurrency}: {results:?}"
        );
        for job in &jobs {
            assert!(
                job.output.exists(),
                "{} was not written",
                job.output.display()
            );
        }
    }
}

#[test]
fn the_default_concurrency_is_at_least_one() {
    assert!(convert::default_concurrency() >= 1);
}

/// A folder of a hundred tracks must not sit silent until the last one lands.
/// Completions are reported as they happen, which is also what a progress bar
/// will need later.
#[test]
fn each_completion_is_reported_as_it_happens() {
    if !tools_available() {
        return;
    }

    let dir = workspace("convert-parallel-reports");
    let source_path = encode(&dir, "source.wav", 44_100, &["-c:a", "pcm_s16le"]);
    let source = probe::run(Path::new("ffprobe"), &source_path).expect("probe source");
    let to_aiff = plan::plan(&source, &Target::from_format("aiff").expect("aiff"));

    let jobs: Vec<_> = (0..6)
        .map(|index| convert::Job {
            plan: to_aiff,
            input: source_path.clone(),
            output: dir.join(format!("reported-{index}.aiff")),
        })
        .collect();

    let seen = std::sync::Mutex::new(Vec::new());
    let results = convert::run_all(Path::new("ffmpeg"), &jobs, 3, &|index, result| {
        seen.lock()
            .expect("seen lock")
            .push((index, result.is_ok()));
    });

    let mut reported = seen.into_inner().expect("seen lock");
    reported.sort_unstable();

    assert_eq!(reported.len(), jobs.len(), "not every job was reported");
    assert!(reported.iter().all(|(_, ok)| *ok), "{reported:?}");
    assert_eq!(results.len(), jobs.len());
}

/// A copy has to leave the bytes alone. Re-encoding a file that already matches
/// would spend time to produce something slightly worse.
#[test]
fn a_copy_reproduces_the_source_byte_for_byte() {
    if !tools_available() {
        return;
    }

    let dir = workspace("convert-copy");
    let source_path = encode(
        &dir,
        "already.mp3",
        44_100,
        &["-c:a", "libmp3lame", "-b:a", "320k"],
    );
    let source = probe::run(Path::new("ffprobe"), &source_path).expect("probe source");

    // Nothing to rewrite in the tags either, which is what leaves a plain copy
    // as the right answer.
    let target = Target {
        metadata: MetadataPolicy {
            clear: &[],
            artwork: Artwork::Keep,
        },
        ..Target::CDJ_SAFE
    };
    let copy = plan::plan(&source, &target);
    assert_eq!(copy.action, Action::Copy);

    let output_path = dir.join("copied.mp3");
    convert::run(Path::new("ffmpeg"), &copy, &source_path, &output_path).expect("convert");

    let before = std::fs::read(&source_path).expect("read source");
    let after = std::fs::read(&output_path).expect("read output");
    assert_eq!(before, after, "a copy changed the file");
}
