use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};

use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use transcrate_core::convert::ConvertError;
use transcrate_core::plan::{self, Action, Artwork, MetadataPolicy, Target};
use transcrate_core::{
    AudioSpec, Codec, DEVICES, DeviceProfile, FileSystem, Issue, Support, by_id, check, convert,
    probe,
};

/// Shown under the top-level help. Someone reading `-h` for the first time
/// wants a line they can paste, not a list of flags to assemble one from.
const EXAMPLES: &str = "\
Examples:
  transcrate convert ~/Music/*.flac          Convert for any player (MP3 320 kbps)
  transcrate convert track.wav --to aiff     Change format, keep rate and depth
  transcrate convert ~/Music -p lossless     Lossless, still playable everywhere
  transcrate check ~/Music/*.mp3             Ask which players will take them
  transcrate check track.flac -d xdj-rr      Ask about one player
  transcrate devices                         List the players checked against

Converted files land in a _transcrate folder beside each input. The source is
never written to.";

#[derive(Debug, Parser)]
#[command(
    name = "transcrate",
    version,
    about = "Convert tracks for your USB, and know they will play before you get to the club.",
    long_about = "Convert tracks for your USB, and know they will play before you get to the club.\n\n\
                  Transcrate converts audio with ffmpeg and checks the result against what CDJs and \
                  XDJs actually accept: codecs, sample rates, bit depths and filesystems, taken from \
                  the manufacturers' manuals.",
    after_help = EXAMPLES,
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Show the players that compatibility is checked against.
    Devices,

    /// Report which players will play the given files.
    ///
    /// Reads each file's real format with ffprobe rather than trusting its
    /// extension, then lists the players that accept it and the reason each of
    /// the others does not. Exits non-zero if anything is rejected.
    #[command(after_help = "\
Examples:
  transcrate check ~/Music/*.flac              Against every player
  transcrate check track.wav -d cdj-3000       Against one
  transcrate check ~/Music/* -d xdj-rr,xdj-xz  Against the gear at tonight's venue")]
    Check {
        /// Files to inspect.
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

        /// Show only the files at least one player rejects.
        #[arg(short, long)]
        failing: bool,

        /// The ffprobe binary to use.
        #[arg(long, default_value = "ffprobe", value_name = "PATH", value_hint = ValueHint::FilePath)]
        ffprobe: PathBuf,
    },

    /// Convert files into a profile's format.
    ///
    /// A profile carries limits with it: cdj-safe fixes the rate at 44.1 kHz so
    /// every player in the table accepts the result. Naming a format with --to
    /// changes only the container and keeps the source's rate and depth, which
    /// is a different question, so the two cannot be combined.
    ///
    /// Files already in the target format are copied rather than re-encoded.
    /// Reducing bit depth adds dither; resampling does not.
    #[command(after_help = "\
Examples:
  transcrate convert ~/Music/*.flac         MP3 320 kbps, plays anywhere
  transcrate convert track.wav --to aiff    AIFF at the source's rate and depth
  transcrate convert ~/Music -p archive     FLAC, nothing changed, for storage
  transcrate convert ~/Music/* -o /Volumes/USB   Straight onto the stick
  transcrate convert ~/Music/* -j 4         Leave some cores alone")]
    Convert(ConvertArgs),

    /// Tidy up tags without touching the audio.
    ///
    /// Every file comes out in the format it went in as, so a folder holding
    /// MP3 next to AIFF takes one command. The audio stream is copied across
    /// untouched: a lossy file loses nothing to a change of text.
    #[command(after_help = "\
Examples:
  transcrate retag ~/Music                      Clear the comment and the lyrics
  transcrate retag ~/Music --no-artwork         Drop the sleeve as well
  transcrate retag ~/Music --keep-comment --no-artwork
                                                Drop the sleeve, keep your notes")]
    Retag(RetagArgs),

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

#[derive(Debug, clap::Args)]
struct ConvertArgs {
    /// Files to convert.
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

    /// Drop embedded artwork instead of carrying it across.
    #[arg(long)]
    no_artwork: bool,

    /// Keep the comment field, which is otherwise emptied.
    ///
    /// Cleared by default because that is where shops and rippers leave their
    /// advertising, and a CDJ shows it next to the title. Worth keeping if you
    /// put your own cue notes or a Camelot key there.
    #[arg(long)]
    keep_comment: bool,

    /// How many files to convert at once. Defaults to one per core.
    #[arg(short = 'j', long, value_name = "N")]
    jobs: Option<usize>,

    /// The ffmpeg binary to use.
    #[arg(long, default_value = "ffmpeg", value_name = "PATH", value_hint = ValueHint::FilePath)]
    ffmpeg: PathBuf,

    /// The ffprobe binary to use.
    #[arg(long, default_value = "ffprobe", value_name = "PATH", value_hint = ValueHint::FilePath)]
    ffprobe: PathBuf,
}

#[derive(Debug, clap::Args)]
struct RetagArgs {
    /// Files to retag.
    #[arg(required = true, value_name = "FILE", value_hint = ValueHint::FilePath)]
    files: Vec<PathBuf>,

    /// Where to write. Defaults to a `_transcrate` folder beside each input.
    #[arg(short, long, value_name = "DIR", value_hint = ValueHint::DirPath)]
    output: Option<PathBuf>,

    /// Drop embedded artwork instead of carrying it across.
    #[arg(long)]
    no_artwork: bool,

    /// Keep the comment field, which is otherwise emptied.
    #[arg(long)]
    keep_comment: bool,

    /// How many files to work on at once. Defaults to one per core.
    #[arg(short = 'j', long, value_name = "N")]
    jobs: Option<usize>,

    /// The ffmpeg binary to use.
    #[arg(long, default_value = "ffmpeg", value_name = "PATH", value_hint = ValueHint::FilePath)]
    ffmpeg: PathBuf,

    /// The ffprobe binary to use.
    #[arg(long, default_value = "ffprobe", value_name = "PATH", value_hint = ValueHint::FilePath)]
    ffprobe: PathBuf,
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
            failing,
            ffprobe,
        } => run_check(&files, &devices, failing, &ffprobe),
        Command::Convert(args) => run_convert(&args),
        Command::Retag(args) => run_retag(&args),
        Command::Completions { shell } => {
            write_completions(shell, &mut std::io::stdout());
            ExitCode::SUCCESS
        }
    }
}

/// Exits non-zero if any file failed, so a partial run is visible to a script
/// without reading the output.
fn run_convert(args: &ConvertArgs) -> ExitCode {
    let target = match resolve_target(args.profile.as_deref(), args.to.as_deref()) {
        Ok(target) => target,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let base = if args.keep_comment {
        MetadataPolicy::KEEPING_COMMENTS
    } else {
        target.metadata
    };

    let target = Target {
        metadata: MetadataPolicy {
            artwork: if args.no_artwork {
                Artwork::Remove
            } else {
                base.artwork
            },
            ..base
        },
        ..target
    };

    run_jobs(
        &args.files,
        args.output.as_deref(),
        args.jobs,
        (args.ffmpeg.as_path(), args.ffprobe.as_path()),
        &|_| target,
    )
}

/// Rewrite tags, leaving every file in the format it arrived in.
fn run_retag(args: &RetagArgs) -> ExitCode {
    let metadata = MetadataPolicy {
        artwork: if args.no_artwork {
            Artwork::Remove
        } else {
            Artwork::Keep
        },
        ..if args.keep_comment {
            MetadataPolicy::KEEPING_COMMENTS
        } else {
            MetadataPolicy::DJ
        }
    };

    run_jobs(
        &args.files,
        args.output.as_deref(),
        args.jobs,
        (args.ffmpeg.as_path(), args.ffprobe.as_path()),
        // Built per file rather than chosen once: that is what lets a folder of
        // mixed formats go through in one pass.
        &|source| Target::keeping(source, metadata),
    )
}

/// Plan every input, run the lot, and report as each lands.
///
/// `target_for` is handed each file's own format, so a caller can either fix
/// the target in advance or derive it from what is there.
fn run_jobs(
    files: &[PathBuf],
    into: Option<&Path>,
    concurrency: Option<usize>,
    tools: (&Path, &Path),
    target_for: &dyn Fn(&AudioSpec) -> Target,
) -> ExitCode {
    let (ffmpeg, ffprobe) = tools;

    // Plan everything before encoding anything, so a file that cannot be read
    // is named straight away rather than after minutes of work on the rest.
    let inputs = collect_inputs(files, PreviousOutput::Skip);
    if inputs.is_empty() {
        eprintln!("no audio files among the paths given");
        return ExitCode::FAILURE;
    }

    let mut planned = Vec::new();
    let mut all_done = true;

    for input in &inputs {
        match prepare(input, into, ffprobe, target_for) {
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
                Action::Retag => "tags rewritten, audio untouched",
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
    input: &Path,
    into: Option<&Path>,
    ffprobe: &Path,
    target_for: &dyn Fn(&AudioSpec) -> Target,
) -> Result<convert::Job, String> {
    let source =
        probe::run(ffprobe, input).map_err(|error| format!("{}: {error}", input.display()))?;
    let plan = plan::plan(&source, &target_for(&source));
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

/// The containers this program reads, used both to sweep a folder and to build
/// the shell completion. One list so the two cannot drift apart.
const AUDIO_EXTENSIONS: [&str; 8] = ["wav", "flac", "aif", "aiff", "m4a", "mp3", "aac", "mp4"];

/// Where converted files go.
const OUTPUT_FOLDER: &str = "_transcrate";

/// Whether a sweep descends into a previous run's output folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviousOutput {
    /// Converting: taking it back in would re-encode the last run's results,
    /// and for a lossy format that means losing a little more each time.
    Skip,
    /// Checking: "did what I made come out playable" is the obvious question to
    /// ask of a folder of conversions, so it has to be answerable.
    Include,
}

/// Expand directories into the audio inside them, recursively.
///
/// One path on its own is taken at its word, whatever it is called: someone who
/// typed a single filename meant that file, and ffprobe judges it better than
/// the extension does. Several at once is almost always a glob the shell
/// expanded, and a shell hands over the artwork and the playlists too — so
/// there, only audio comes through.
fn collect_inputs(paths: &[PathBuf], previous: PreviousOutput) -> Vec<PathBuf> {
    let expanded_by_the_shell = paths.len() > 1;
    let mut found = Vec::new();

    for path in paths {
        if path.is_dir() {
            sweep(path, previous, &mut found);
        } else if !expanded_by_the_shell || is_audio(path) {
            found.push(path.clone());
        }
    }

    // read_dir returns entries in whatever order the filesystem holds them,
    // which would make the report jump around between runs.
    found.sort();
    found
}

fn sweep(directory: &Path, previous: PreviousOutput, into: &mut Vec<PathBuf>) {
    let is_previous_output = directory
        .file_name()
        .is_some_and(|name| name == OUTPUT_FOLDER);

    if is_previous_output && previous == PreviousOutput::Skip {
        return;
    }

    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sweep(&path, previous, into);
        } else if is_audio(&path) {
            into.push(path);
        }
    }
}

fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            AUDIO_EXTENSIONS
                .iter()
                .any(|known| known.eq_ignore_ascii_case(ext))
        })
}

/// The zsh glob for the same set. `(#i)` makes it case-insensitive, so a `.WAV`
/// ripped years ago still shows up.
fn audio_glob() -> String {
    format!("(#i)*.({})", AUDIO_EXTENSIONS.join("|"))
}

fn write_completions(shell: Shell, out: &mut impl std::io::Write) {
    let mut script = Vec::new();
    clap_complete::generate(shell, &mut Cli::command(), "transcrate", &mut script);

    // zsh's `_files -g` narrows the offer to audio while still listing
    // directories, which is what makes it navigable. clap_complete cannot
    // express that, so the generated lines are rewritten here.
    //
    // Matched by position rather than by the whole line: clap builds the line
    // from the argument's help text, so pinning the exact string would break
    // silently the next time that wording changed. Only the positional
    // arguments are touched — --ffmpeg and --ffprobe name binaries, and -o
    // names a directory.
    if shell == Shell::Zsh {
        let glob = audio_glob();
        let narrowed: Vec<_> = String::from_utf8_lossy(&script)
            .lines()
            .map(|line| {
                if line.trim_start().starts_with("'*::files") {
                    line.replace(":_files'", &format!(":_files -g \"{glob}\"'"))
                } else {
                    line.to_owned()
                }
            })
            .collect();
        script = narrowed.join("\n").into_bytes();
    }

    out.write_all(&script).expect("write completion script");
}

/// Exits non-zero when any file fails to read or any named player rejects one,
/// so this can gate a script without parsing the output.
fn run_check(
    files: &[PathBuf],
    device_ids: &[String],
    failing_only: bool,
    ffprobe: &Path,
) -> ExitCode {
    let players = match resolve_players(device_ids) {
        Ok(players) => players,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    let inputs = collect_inputs(files, PreviousOutput::Include);
    if inputs.is_empty() {
        eprintln!("no audio files among the paths given");
        return ExitCode::FAILURE;
    }

    let mut all_clear = true;
    let mut rejected_count = 0usize;
    let progress = Progress::new(inputs.len());

    for (index, file) in inputs.iter().enumerate() {
        progress.show(index + 1);
        let outcome = probe::run(ffprobe, file);
        // Anything printed has to appear above the counter, not through it.
        progress.clear();

        match outcome {
            Ok(spec) => {
                let failing = rejected_anywhere(&spec, &players);
                if failing {
                    rejected_count += 1;
                    all_clear = false;
                }
                // With --failing, a clean file is one you do not need to see;
                // the point of the flag is to leave only what needs doing.
                if failing || !failing_only {
                    report(file, &spec, &players);
                }
            }
            Err(error) => {
                eprintln!("{}: {error}", file.display());
                rejected_count += 1;
                all_clear = false;
            }
        }
    }

    progress.clear();

    // One file speaks for itself; a folder needs a count, or a clean run under
    // --failing prints nothing at all and looks like it did not work.
    if inputs.len() > 1 {
        println!("{rejected_count} of {} rejected", inputs.len());
    }

    if all_clear {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A counter that overwrites its own line on stderr.
///
/// stderr rather than stdout so the report can still be piped somewhere, and
/// only to a terminal: redirected to a file, this would collect a carriage
/// return per track and nothing readable.
struct Progress {
    total: usize,
    to_a_terminal: bool,
}

impl Progress {
    fn new(total: usize) -> Self {
        use std::io::IsTerminal;
        Self {
            total,
            to_a_terminal: std::io::stderr().is_terminal(),
        }
    }

    fn show(&self, done: usize) {
        use std::io::Write;
        if !self.to_a_terminal {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "{}", progress_line(done, self.total));
        let _ = err.flush();
    }

    /// Wipe the counter before anything is printed above it, so a result never
    /// lands on top of a half-written line.
    fn clear(&self) {
        use std::io::Write;
        if !self.to_a_terminal {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "\r\x1b[K");
        let _ = err.flush();
    }
}

/// `\r` returns to column zero and `\x1b[K` wipes what was there, so a shorter
/// count does not leave the tail of a longer one behind.
fn progress_line(done: usize, total: usize) -> String {
    format!("\r\x1b[Kchecking {done}/{total}")
}

/// Whether any of `players` refuses this file.
///
/// Any, not every: a track that plays on nine of ten is still the one that
/// stops the set.
fn rejected_anywhere(spec: &AudioSpec, players: &[&'static DeviceProfile]) -> bool {
    players.iter().any(|player| !check(spec, player).is_empty())
}

/// Print one file's verdict.
fn report(file: &Path, spec: &AudioSpec, players: &[&'static DeviceProfile]) {
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

        // Every subcommand taking files positionally gets the narrowed offer.
        // Counted rather than named, so adding a subcommand that misses out
        // fails here instead of shipping a wider completion than intended.
        let positional = script
            .lines()
            .filter(|line| line.trim_start().starts_with("'*::files"))
            .count();
        let narrowed = script
            .lines()
            .filter(|line| line.contains("_files -g"))
            .count();

        assert!(positional >= 2, "expected check and convert at least");
        assert_eq!(narrowed, positional, "a file argument was left unnarrowed");
        assert!(
            script.contains(&audio_glob()),
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

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("transcrate-cli-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    /// Pointing at a folder is the common case — nobody wants to name four
    /// hundred tracks, and a shell glob does not reach into subfolders.
    #[test]
    fn a_directory_expands_to_the_audio_inside_it() {
        let dir = scratch("collect");
        std::fs::create_dir_all(dir.join("sub")).expect("subdir");
        std::fs::write(dir.join("a.wav"), b"").expect("write");
        std::fs::write(dir.join("sub/b.flac"), b"").expect("write");
        std::fs::write(dir.join("cover.jpg"), b"").expect("write");
        std::fs::write(dir.join("notes.txt"), b"").expect("write");

        let names: Vec<_> = collect_inputs(&[dir], PreviousOutput::Skip)
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["a.wav", "b.flac"]);
    }

    /// Converting a folder twice must not convert the first run's output: that
    /// would re-encode a lossy file into itself, losing a little more each time.
    /// Checking one should look at it, though — "did what I made come out
    /// playable" is the obvious question to ask of a folder of conversions.
    #[test]
    fn only_converting_skips_a_previous_runs_output() {
        let dir = scratch("collect-previous-output");
        std::fs::create_dir_all(dir.join("_transcrate")).expect("subdir");
        std::fs::write(dir.join("track.wav"), b"").expect("write");
        std::fs::write(dir.join("_transcrate/track.mp3"), b"").expect("write");

        let names = |previous| {
            collect_inputs(std::slice::from_ref(&dir), previous)
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };

        assert_eq!(names(PreviousOutput::Skip), ["track.wav"]);
        assert_eq!(names(PreviousOutput::Include), ["track.mp3", "track.wav"]);
    }

    /// One path named on its own is taken at its word, whatever it is called.
    /// Someone who typed a single filename meant that file, and ffprobe is a
    /// better judge of it than the extension.
    #[test]
    fn a_single_named_file_is_taken_as_given() {
        let dir = scratch("collect-explicit");
        let odd = dir.join("no-extension");
        std::fs::write(&odd, b"").expect("write");

        assert_eq!(
            collect_inputs(std::slice::from_ref(&odd), PreviousOutput::Skip),
            vec![odd]
        );
    }

    /// `transcrate convert *` is the obvious way to do a folder from the shell,
    /// and the shell hands over everything in it: artwork, playlists, notes.
    /// Reporting each of those as a failure would bury the conversions.
    #[test]
    fn several_paths_at_once_keep_only_the_audio() {
        let dir = scratch("collect-glob");
        let expanded: Vec<_> = ["a.wav", "b.flac", "cover.jpg", "playlist.m3u", "notes.txt"]
            .iter()
            .map(|name| {
                let path = dir.join(name);
                std::fs::write(&path, b"").expect("write");
                path
            })
            .collect();

        let names: Vec<_> = collect_inputs(&expanded, PreviousOutput::Skip)
            .iter()
            .filter_map(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, ["a.wav", "b.flac"]);
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

    /// The counter has to overwrite itself rather than scroll, or a folder of
    /// two hundred leaves two hundred lines of progress above the report.
    #[test]
    fn progress_returns_to_the_start_of_its_own_line() {
        let line = progress_line(3, 10);

        assert!(
            line.starts_with('\r'),
            "does not return to column zero: {line:?}"
        );
        assert!(line.contains("3/10"), "does not say where it is: {line:?}");
        assert!(!line.contains('\n'), "a newline would scroll it: {line:?}");
    }

    /// "Any player rejects it", not "every player rejects it". A track that
    /// plays on nine of ten is still the one that stops the set, so it has to
    /// come through the filter.
    #[test]
    fn a_file_is_failing_when_any_player_rejects_it() {
        let everywhere = resolve_players(&[]).expect("all players");

        // FLAC at 96 kHz: fine on a CDJ-3000, refused outright by an XDJ-RR.
        let hi_res = AudioSpec {
            codec: Codec::Flac,
            sample_rate_hz: 96_000,
            bit_depth: Some(24),
            bitrate_kbps: None,
        };
        assert!(rejected_anywhere(&hi_res, &everywhere));

        // The default profile's output, which every player takes.
        let safe = AudioSpec {
            codec: Codec::Mp3,
            sample_rate_hz: 44_100,
            bit_depth: None,
            bitrate_kbps: Some(320),
        };
        assert!(!rejected_anywhere(&safe, &everywhere));

        // Narrowing to the players that do take it clears the same file.
        let modern = resolve_players(&["cdj-3000".to_owned()]).expect("cdj-3000");
        assert!(!rejected_anywhere(&hi_res, &modern));
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
