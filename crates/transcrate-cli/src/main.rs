use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use transcrate_core::convert::ConvertError;
use transcrate_core::plan::{self, Action, Target};
use transcrate_core::{
    AudioSpec, Codec, DEVICES, DeviceProfile, FileSystem, Issue, Support, by_id, check, convert,
    probe,
};

/// Fast, DJ-oriented audio transcoder built on ffmpeg.
#[derive(Debug, Parser)]
#[command(name = "transcrate", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the players that compatibility is checked against.
    Devices,

    /// Report which players will play the given files.
    Check {
        #[arg(required = true, value_name = "FILE", value_hint = ValueHint::FilePath)]
        files: Vec<PathBuf>,

        /// Players to check against, comma-separated. Defaults to all of them.
        #[arg(
            short = 'd',
            long = "device",
            value_name = "ID",
            value_delimiter = ',',
            value_parser = PossibleValuesParser::new(DEVICES.iter().map(|player| player.id)),
        )]
        devices: Vec<String>,

        /// The ffprobe binary to use.
        #[arg(long, default_value = "ffprobe", value_name = "PATH", value_hint = ValueHint::FilePath)]
        ffprobe: PathBuf,
    },

    /// Convert files into a profile's format.
    Convert {
        #[arg(required = true, value_name = "FILE", value_hint = ValueHint::FilePath)]
        files: Vec<PathBuf>,

        /// Profile to convert into. Defaults to cdj-safe.
        #[arg(
            short,
            long,
            conflicts_with = "to",
            value_parser = PossibleValuesParser::new(Target::NAMES),
        )]
        profile: Option<String>,

        /// Convert into this format, keeping the source's rate and depth.
        #[arg(
            long,
            value_name = "FORMAT",
            value_parser = PossibleValuesParser::new(Target::FORMATS),
        )]
        to: Option<String>,

        /// Where to write. Defaults to a `_transcrate` folder beside each input.
        #[arg(short, long, value_name = "DIR", value_hint = ValueHint::DirPath)]
        output: Option<PathBuf>,

        /// How many files to convert at once. Defaults to one per core.
        #[arg(short = 'j', long, value_name = "N")]
        jobs: Option<usize>,

        /// The ffmpeg binary to use.
        #[arg(long, default_value = "ffmpeg", value_name = "PATH", value_hint = ValueHint::FilePath)]
        ffmpeg: PathBuf,

        /// The ffprobe binary to use.
        #[arg(long, default_value = "ffprobe", value_name = "PATH", value_hint = ValueHint::FilePath)]
        ffprobe: PathBuf,
    },

    /// Print a shell completion script.
    ///
    /// For zsh, write it somewhere on your fpath:
    ///
    ///     transcrate completions zsh > "${fpath[1]}/_transcrate"
    Completions {
        /// Shell to generate the script for.
        shell: Shell,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Devices => {
            print_devices();
            ExitCode::SUCCESS
        }
        Command::Check {
            files,
            devices,
            ffprobe,
        } => run_check(&files, &devices, &ffprobe),
        Command::Convert {
            files,
            profile,
            to,
            output,
            jobs,
            ffmpeg,
            ffprobe,
        } => run_convert(
            &files,
            profile.as_deref(),
            to.as_deref(),
            output.as_deref(),
            jobs,
            &ffmpeg,
            &ffprobe,
        ),
        Command::Completions { shell } => {
            write_completions(shell, &mut std::io::stdout());
            ExitCode::SUCCESS
        }
    }
}

/// Exits non-zero if any file failed, so a partial run is visible to a script
/// without reading the output.
fn run_convert(
    files: &[PathBuf],
    profile: Option<&str>,
    to: Option<&str>,
    into: Option<&Path>,
    concurrency: Option<usize>,
    ffmpeg: &Path,
    ffprobe: &Path,
) -> ExitCode {
    let target = match resolve_target(profile, to) {
        Ok(target) => target,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    // Plan everything before encoding anything, so a file that cannot be read
    // is named straight away rather than after minutes of work on the rest.
    let mut planned = Vec::new();
    let mut all_done = true;

    for input in files {
        match prepare(&target, input, into, ffprobe) {
            Ok(job) => planned.push(job),
            Err(message) => {
                eprintln!("{message}");
                all_done = false;
            }
        }
    }

    let total = planned.len();
    let done = AtomicUsize::new(0);

    let results = convert::run_all(
        ffmpeg,
        &planned,
        concurrency.unwrap_or_else(convert::default_concurrency),
        &|index, result| {
            let finished = done.fetch_add(1, Ordering::Relaxed) + 1;
            report_one(finished, total, &planned[index], result);
        },
    );

    all_done &= results.iter().all(Result::is_ok);

    if all_done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// One line per file, written under a held lock so parallel workers cannot
/// interleave halfway through a line.
fn report_one(
    finished: usize,
    total: usize,
    job: &convert::Job,
    result: &Result<(), ConvertError>,
) {
    use std::io::Write;

    let name = job.input.file_name().unwrap_or(job.input.as_os_str());

    match result {
        Ok(()) => {
            let how = match job.plan.action {
                Action::Copy => "copied",
                Action::Encode { dither: true } => "encoded, dithered",
                Action::Encode { dither: false } => "encoded",
            };
            let mut out = std::io::stdout().lock();
            let _ = writeln!(
                out,
                "[{finished}/{total}] {} -> {}  ({how})",
                name.display(),
                enclosing_path(&job.output)
            );
        }
        Err(error) => {
            let mut err = std::io::stderr().lock();
            let _ = writeln!(err, "[{finished}/{total}] {}: {error}", name.display());
        }
    }
}

/// A path as its own folder and name, which says where a file went without
/// repeating the whole library path on every line.
fn enclosing_path(path: &Path) -> String {
    let name = path.file_name().unwrap_or(path.as_os_str());

    match path.parent().and_then(Path::file_name) {
        Some(folder) => format!("{}/{}", folder.display(), name.display()),
        None => name.display().to_string(),
    }
}

fn prepare(
    target: &Target,
    input: &Path,
    into: Option<&Path>,
    ffprobe: &Path,
) -> Result<convert::Job, String> {
    let source =
        probe::run(ffprobe, input).map_err(|error| format!("{}: {error}", input.display()))?;
    let plan = plan::plan(&source, target);
    let output = output_path(input, into, plan.output.codec)?;

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
    }

    Ok(convert::Job {
        plan,
        input: input.to_path_buf(),
        output,
    })
}

/// Work out what to convert into.
///
/// clap rules out naming both, so this only has to decide between one, the
/// other, and neither.
fn resolve_target(profile: Option<&str>, to: Option<&str>) -> Result<Target, String> {
    match (profile, to) {
        (Some(name), _) => Target::by_name(name).ok_or_else(|| {
            format!(
                "unknown profile: {name}. Try one of: {}",
                Target::NAMES.join(", ")
            )
        }),
        (None, Some(format)) => Target::from_format(format).ok_or_else(|| {
            format!(
                "unknown format: {format}. Try one of: {}",
                Target::FORMATS.join(", ")
            )
        }),
        (None, None) => Ok(Target::CDJ_SAFE),
    }
}

/// Where a converted file lands.
///
/// Defaults to a `_transcrate` folder beside the input, so results sit next to
/// the tracks they came from and never inside the source library itself.
fn output_path(input: &Path, into: Option<&Path>, codec: Codec) -> Result<PathBuf, String> {
    let stem = input
        .file_stem()
        .ok_or_else(|| format!("{} has no file name", input.display()))?;

    let directory = match into {
        Some(dir) => dir.to_path_buf(),
        None => input.parent().unwrap_or(Path::new(".")).join("_transcrate"),
    };

    let mut destination = directory.join(stem);
    destination.set_extension(extension_for(codec));

    if destination == input {
        return Err(format!(
            "refusing to overwrite the source: {}",
            input.display()
        ));
    }

    Ok(destination)
}

const fn extension_for(codec: Codec) -> &'static str {
    match codec {
        Codec::Mp3 => "mp3",
        // The same ambiguity the reader has to cope with: both live in .m4a.
        Codec::AacLc | Codec::Alac => "m4a",
        Codec::Flac => "flac",
        Codec::PcmWav => "wav",
        Codec::PcmAiff => "aiff",
    }
}

/// The containers this program reads. `(#i)` makes the match case-insensitive,
/// so a `.WAV` ripped years ago still shows up.
const AUDIO_GLOB: &str = "(#i)*.(wav|flac|aif|aiff|m4a|mp3|aac|mp4)";

fn write_completions(shell: Shell, out: &mut impl std::io::Write) {
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "transcrate", &mut script);

    // zsh's `_files -g` narrows the offer to audio while still listing
    // directories, which is what makes it navigable. clap_complete has no way
    // to express that, so the generated line is rewritten here. Only the
    // positional argument is touched: --ffprobe names a binary.
    if shell == Shell::Zsh {
        let narrowed = String::from_utf8_lossy(&script).replace(
            "'*::files:_files'",
            &format!("'*::files:_files -g \"{AUDIO_GLOB}\"'"),
        );
        script = narrowed.into_bytes();
    }

    out.write_all(&script).expect("write completion script");
}

/// Exits non-zero when any file fails to read or any named player rejects one,
/// so this can gate a script without parsing the output.
fn run_check(files: &[PathBuf], device_ids: &[String], ffprobe: &Path) -> ExitCode {
    let players = match resolve_players(device_ids) {
        Ok(players) => players,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let mut all_clear = true;

    for file in files {
        match probe::run(ffprobe, file) {
            Ok(spec) => all_clear &= report(file, &spec, &players),
            Err(error) => {
                eprintln!("{}: {error}", file.display());
                all_clear = false;
            }
        }
    }

    if all_clear {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Print one file's verdict, returning whether every player accepted it.
fn report(file: &Path, spec: &AudioSpec, players: &[&'static DeviceProfile]) -> bool {
    println!("{}", file.display());
    println!("  {}", describe_spec(spec));

    let mut playable = Vec::new();
    let mut rejected = Vec::new();

    for player in players {
        let issues = check(spec, player);
        if issues.is_empty() {
            playable.push(player.display_name);
        } else {
            rejected.push((player.display_name, issues));
        }
    }

    if !playable.is_empty() {
        println!("  plays on       {}", playable.join(", "));
    }
    for (name, issues) in &rejected {
        let reasons: Vec<_> = issues.iter().copied().map(describe_issue).collect();
        println!("  {name:<14} {}", reasons.join("; "));
    }
    println!();

    rejected.is_empty()
}

/// Resolve player ids, where naming none means all of them.
fn resolve_players(ids: &[String]) -> Result<Vec<&'static DeviceProfile>, String> {
    if ids.is_empty() {
        return Ok(DEVICES.iter().collect());
    }

    ids.iter()
        .map(|id| {
            by_id(id).ok_or_else(|| {
                format!("unknown player: {id}. Run `transcrate devices` for the list.")
            })
        })
        .collect()
}

fn describe_spec(spec: &AudioSpec) -> String {
    let mut parts = vec![
        codec_name(spec.codec).to_owned(),
        format!("{} kHz", khz(spec.sample_rate_hz)),
    ];
    if let Some(bits) = spec.bit_depth {
        parts.push(format!("{bits}-bit"));
    }
    if let Some(kbps) = spec.bitrate_kbps {
        parts.push(format!("{kbps} kbps"));
    }
    parts.join(" ")
}

/// Name the value to change, not the rule it broke.
fn describe_issue(issue: Issue) -> String {
    match issue {
        Issue::CodecUnsupported { codec } => {
            format!("{} is not supported", codec_name(codec))
        }
        Issue::SampleRateUnsupported {
            codec,
            requested_hz,
        } => {
            format!(
                "{} kHz is not supported for {}",
                khz(requested_hz),
                codec_name(codec)
            )
        }
        Issue::BitDepthUnsupported {
            codec,
            requested_bits,
        } => {
            format!(
                "{requested_bits}-bit is not supported for {}",
                codec_name(codec)
            )
        }
        Issue::BitrateOutOfRange {
            codec,
            requested_kbps,
            allowed_kbps: (min, max),
        } => {
            format!(
                "{requested_kbps} kbps is outside {min}-{max} kbps for {}",
                codec_name(codec)
            )
        }
    }
}

fn print_devices() {
    println!(
        "{:<14}{:>5}  {:>6}{:>6}{:>7}{:>7}{:>7}{:>7}  EXFAT",
        "DEVICE", "YEAR", "MP3", "AAC", "WAV", "AIFF", "FLAC", "ALAC"
    );

    for device in DEVICES {
        println!(
            "{:<14}{:>5}  {:>6}{:>6}{:>7}{:>7}{:>7}{:>7}  {}",
            device.display_name,
            device.release_year,
            max_rate(device, Codec::Mp3),
            max_rate(device, Codec::AacLc),
            max_rate(device, Codec::PcmWav),
            max_rate(device, Codec::PcmAiff),
            max_rate(device, Codec::Flac),
            max_rate(device, Codec::Alac),
            describe(device.filesystem_support(FileSystem::ExFat)),
        );
    }

    println!("\nRates are the highest documented sampling frequency; '-' means unsupported.");
    println!("Source: manufacturer operating instructions, see docs/device-compatibility.md.");
}

/// Highest documented sampling rate for a codec, or `-` when the player cannot
/// play it at all.
fn max_rate(device: &DeviceProfile, codec: Codec) -> String {
    device
        .formats_for(codec)
        .flat_map(|format| format.sample_rates_hz)
        .max()
        .map_or_else(|| "-".to_owned(), |hz| format!("{}k", khz(*hz)))
}

/// Sampling rate in kHz with no unit: `48`, `44.1`.
fn khz(hz: u32) -> String {
    if hz.is_multiple_of(1_000) {
        (hz / 1_000).to_string()
    } else {
        format!("{:.1}", f64::from(hz) / 1_000.0)
    }
}

fn codec_name(codec: Codec) -> &'static str {
    match codec {
        Codec::Mp3 => "MP3",
        Codec::AacLc => "AAC",
        Codec::Alac => "ALAC",
        Codec::Flac => "FLAC",
        Codec::PcmWav => "WAV",
        Codec::PcmAiff => "AIFF",
    }
}

fn describe(support: Support) -> &'static str {
    match support {
        Support::Yes => "yes",
        Support::No => "no",
        Support::Unknown => "undocumented",
        Support::Conflicting => "sources disagree",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// clap's own check for a contradictory definition, which otherwise only
    /// shows up as a panic at run time.
    #[test]
    fn the_cli_definition_is_internally_consistent() {
        Cli::command().debug_assert();
    }

    /// Player ids exist to be offered, not memorised. Registering them as
    /// possible values gets them into the shell completion and lets clap reject
    /// a typo before any file is opened.
    #[test]
    fn player_ids_are_offered_as_completions() {
        let command = Cli::command();
        let check = command.find_subcommand("check").expect("check subcommand");
        let device = check
            .get_arguments()
            .find(|arg| arg.get_id() == "devices")
            .expect("device argument");

        let offered: Vec<_> = device
            .get_possible_values()
            .iter()
            .map(|value| value.get_name().to_owned())
            .collect();

        assert_eq!(offered.len(), DEVICES.len());
        assert!(offered.contains(&"xdj-rr".to_owned()), "got: {offered:?}");
    }

    /// Completing every file on disk buries the handful that can be converted.
    /// Directories still complete, or there would be no way to walk into one.
    #[test]
    fn file_completion_offers_audio_and_directories_only() {
        let mut script = Vec::new();
        write_completions(clap_complete::Shell::Zsh, &mut script);
        let script = String::from_utf8(script).expect("utf8");

        assert!(
            script.contains(&format!("'*::files:_files -g \"{AUDIO_GLOB}\"'")),
            "positional file argument is not narrowed"
        );
        assert!(
            script.contains("flac"),
            "audio glob missing from the script"
        );

        // --ffprobe points at a binary, so narrowing it would hide the target.
        assert!(
            script.contains("'--ffprobe=[The ffprobe binary to use]:PATH:_files'"),
            "--ffprobe completion was narrowed too"
        );
    }

    #[test]
    fn a_completion_script_names_the_subcommands() {
        let mut script = Vec::new();
        write_completions(clap_complete::Shell::Zsh, &mut script);
        let script = String::from_utf8(script).expect("utf8");

        assert!(script.contains("transcrate"));
        assert!(script.contains("check"));
        assert!(script.contains("devices"));
    }

    /// A profile carries limits with it and a format does not, so asking for
    /// both is ambiguous rather than additive.
    #[test]
    fn a_profile_and_a_format_cannot_both_be_given() {
        let both = Cli::try_parse_from([
            "transcrate",
            "convert",
            "track.wav",
            "-p",
            "cdj-safe",
            "--to",
            "aiff",
        ]);
        assert!(both.is_err(), "clap accepted both at once");

        let format_only =
            Cli::try_parse_from(["transcrate", "convert", "track.wav", "--to", "aiff"]);
        assert!(format_only.is_ok(), "{:?}", format_only.err());
    }

    #[test]
    fn naming_neither_falls_back_to_the_default_profile() {
        assert_eq!(
            resolve_target(None, None).expect("default"),
            Target::CDJ_SAFE
        );
    }

    #[test]
    fn a_format_resolves_to_a_target_that_changes_nothing_else() {
        let target = resolve_target(None, Some("aiff")).expect("aiff");
        assert_eq!(target, Target::from_format("aiff").expect("aiff"));
    }

    /// Conversions land beside the source rather than in the working directory,
    /// so converting a folder leaves the results next to the tracks they came
    /// from instead of wherever the shell happened to be.
    #[test]
    fn output_lands_in_a_subfolder_beside_the_input() {
        let path = output_path(Path::new("/music/track.flac"), None, Codec::Mp3).expect("path");
        assert_eq!(path, Path::new("/music/_transcrate/track.mp3"));
    }

    #[test]
    fn a_named_directory_takes_the_files_instead() {
        let path = output_path(
            Path::new("/music/track.flac"),
            Some(Path::new("/out")),
            Codec::Mp3,
        )
        .expect("path");
        assert_eq!(path, Path::new("/out/track.mp3"));
    }

    /// Someone's library is not ours to overwrite. Converting an MP3 to MP3
    /// into its own directory would land straight on top of the original, and
    /// no amount of speed makes that worth it.
    #[test]
    fn writing_over_the_source_is_refused() {
        let error = output_path(
            Path::new("/music/track.mp3"),
            Some(Path::new("/music")),
            Codec::Mp3,
        )
        .expect_err("should refuse");

        assert!(error.contains("track.mp3"), "got: {error}");
    }

    /// ALAC and AAC share .m4a, which is the same ambiguity the reader has to
    /// cope with — writing it is where the ambiguity starts.
    #[test]
    fn the_extension_follows_the_codec() {
        let cases = [
            (Codec::Mp3, "mp3"),
            (Codec::AacLc, "m4a"),
            (Codec::Alac, "m4a"),
            (Codec::Flac, "flac"),
            (Codec::PcmWav, "wav"),
            (Codec::PcmAiff, "aiff"),
        ];

        for (codec, expected) in cases {
            let path = output_path(Path::new("/music/track.wv"), Some(Path::new("/out")), codec)
                .expect("path");
            assert_eq!(path.extension().and_then(|e| e.to_str()), Some(expected));
        }
    }

    /// Checking against nothing in particular means checking against everything;
    /// asking "will this play?" without naming a player is the common case.
    #[test]
    fn no_player_named_means_all_of_them() {
        let resolved = resolve_players(&[]).expect("resolve");
        assert_eq!(resolved.len(), DEVICES.len());
    }

    #[test]
    fn players_are_resolved_by_id() {
        let resolved =
            resolve_players(&["xdj-rr".to_owned(), "cdj-3000".to_owned()]).expect("resolve");
        let names: Vec<_> = resolved.iter().map(|player| player.display_name).collect();
        assert_eq!(names, ["XDJ-RR", "CDJ-3000"]);
    }

    /// A typo in a player id must not silently check against nothing, which
    /// would report a clean bill of health for a file nobody looked at.
    #[test]
    fn an_unknown_id_is_an_error_that_says_where_to_look() {
        let error = resolve_players(&["cdj-4000".to_owned()]).expect_err("should reject");
        assert!(error.contains("cdj-4000"));
        assert!(error.contains("transcrate devices"));
    }

    /// The message has to name the value that is wrong and the codec it is
    /// wrong for, since the fix differs between them.
    #[test]
    fn an_issue_reads_as_the_thing_to_change() {
        let rate = describe_issue(Issue::SampleRateUnsupported {
            codec: Codec::Flac,
            requested_hz: 96_000,
        });
        assert!(rate.contains("96 kHz"), "got: {rate}");
        assert!(rate.contains("FLAC"), "got: {rate}");

        let codec = describe_issue(Issue::CodecUnsupported { codec: Codec::Alac });
        assert!(codec.contains("ALAC"), "got: {codec}");

        let depth = describe_issue(Issue::BitDepthUnsupported {
            codec: Codec::PcmWav,
            requested_bits: 32,
        });
        assert!(depth.contains("32"), "got: {depth}");
    }
}
