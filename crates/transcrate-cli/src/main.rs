use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Mutex, PoisonError};

use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use transcrate_core::convert::ConvertError;
use transcrate_core::files::{self, PreviousOutput};
use transcrate_core::plan::{self, Action, Artwork, MetadataPolicy, Target};
use transcrate_core::usb;
use transcrate_core::{
    AudioSpec, Codec, DEVICES, DeviceProfile, FileSystem, Issue, Support, by_id, check, convert,
    parallel, probe, scan,
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
  transcrate retag ~/Music                      Clear the lyrics, keep your notes
  transcrate retag ~/Music --no-artwork         Drop the sleeve as well
  transcrate retag ~/Music --clear-comment      Clear a shop's text out of the
                                                comment, and your notes with it")]
    Retag(RetagArgs),

    /// Check a drive before you take it to a gig.
    ///
    /// Read-only. Nothing here writes to the drive, formats it, or moves a
    /// file: a tool you point at your set on a Friday evening has no business
    /// being able to damage it.
    #[command(after_help = "\
Examples:
  transcrate usb                            List what is plugged in
  transcrate usb /Volumes/DJ                Against every player
  transcrate usb /Volumes/DJ -d xdj-rr      Against the one in tonight's booth")]
    Usb {
        /// The drive, or any path on it. Left out, every drive is listed.
        #[arg(value_name = "PATH", value_hint = ValueHint::DirPath)]
        path: Option<PathBuf>,

        /// Players to check against, comma-separated. Defaults to all of them.
        #[arg(
            short = 'd',
            long = "device",
            value_name = "ID",
            value_delimiter = ',',
            value_parser = PossibleValuesParser::new(DEVICES.iter().map(|player| player.id)),
        )]
        devices: Vec<String>,

        /// Report the filesystem only, without reading the tracks.
        #[arg(long)]
        no_tracks: bool,

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

    /// Empty the comment field, which is otherwise carried across.
    ///
    /// Worth doing on a library bought from shops that fill it with their own
    /// advertising, which a CDJ then shows next to the title. It is left alone
    /// by default, because that is also where DJs put their own cue notes and
    /// Camelot keys.
    #[arg(long)]
    clear_comment: bool,

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

    /// Empty the comment field, which is otherwise carried across.
    #[arg(long)]
    clear_comment: bool,

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
        Command::Usb {
            path,
            devices,
            no_tracks,
            ffprobe,
        } => match path {
            Some(path) => run_usb(&path, &devices, (!no_tracks).then_some(ffprobe.as_path())),
            None => list_drives(),
        },
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

    let base = if args.clear_comment {
        MetadataPolicy::CLEARING_COMMENTS
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
        ..if args.clear_comment {
            MetadataPolicy::CLEARING_COMMENTS
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

/// Name the folders the sweep could not list.
///
/// Whatever audio was behind them is missing from every count that follows, so
/// leaving them unsaid turns a partial run into one that looks complete.
fn report_unreadable(folders: &[PathBuf]) {
    for folder in folders {
        eprintln!(
            "{}: could not be read, and nothing inside it was checked",
            folder.display()
        );
    }
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
    target_for: &(dyn Fn(&AudioSpec) -> Target + Sync),
) -> ExitCode {
    let (ffmpeg, ffprobe) = tools;
    let concurrency = concurrency.unwrap_or_else(parallel::default_concurrency);

    // Plan everything before encoding anything, so a file that cannot be read
    // is named straight away rather than after minutes of work on the rest.
    let found = files::collect(files, PreviousOutput::Skip);
    report_unreadable(&found.unreadable);

    if found.files.is_empty() {
        eprintln!("no audio files among the paths given");
        return ExitCode::FAILURE;
    }

    let prepared = convert::prepare_all(&found, into, ffprobe, target_for, concurrency, &|_, _| {});

    let mut planned = Vec::new();
    // A folder that would not open is a failure of the run, not a detail of it:
    // tracks behind it were never planned and never converted.
    let mut all_done = found.unreadable.is_empty();

    for outcome in prepared {
        match outcome {
            Ok(job) => planned.push(job),
            Err(error) => {
                eprintln!("{error}");
                all_done = false;
            }
        }
    }

    let total = planned.len();
    // Held across the line as well as the count. Numbering the lines and then
    // racing to print them is how [2/3] arrives above [1/3].
    let done = Mutex::new(0usize);

    let results = convert::run_all(ffmpeg, &planned, concurrency, &|index, result| {
        let mut finished = done.lock().unwrap_or_else(PoisonError::into_inner);
        *finished += 1;
        let job = &planned[index];
        // Where the results are rooted, so a line says where the file went
        // rather than naming the one folder it happens to sit in.
        let root = into.or_else(|| found.base_of(&job.input));
        report_one(*finished, total, job, root, result);
    });

    all_done &= results.iter().all(Result::is_ok);

    if all_done {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Name every drive that could be carried to a gig.
///
/// Answering "what is it called and where is it mounted" is most of what stands
/// between plugging a stick in and being able to ask about it — on a Mac the
/// mount point is not something anyone has memorised.
fn list_drives() -> ExitCode {
    let drives = usb::drives();

    if drives.is_empty() {
        eprintln!("nothing removable is plugged in");
        return ExitCode::FAILURE;
    }

    for drive in &drives {
        // Whatever the system called it, where this program has no name of its
        // own for it: an unrecognised filesystem is worth showing as it is.
        let filesystem = match drive.filesystem {
            Some(known) => filesystem_name(known),
            None => drive.reported_as.as_str(),
        };

        println!(
            "{:<20} {:<8} {}",
            drive.name,
            filesystem,
            drive.mount_point.display()
        );
    }

    ExitCode::SUCCESS
}

/// Report which players will read a drive, and what is on it.
///
/// Exits non-zero when any of the named players will not, so this can gate a
/// script — and because a drive half the booth cannot read is a problem worth
/// a failing exit code. Passing `ffprobe` reads the tracks too, which is the
/// slow part and the reason it can be turned off.
fn run_usb(path: &Path, device_ids: &[String], ffprobe: Option<&Path>) -> ExitCode {
    let players = match resolve_players(device_ids) {
        Ok(players) => players,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };

    if !path.exists() {
        eprintln!("{}: no such path", path.display());
        return ExitCode::FAILURE;
    }

    let Some(drive) = usb::drive_at(path) else {
        eprintln!("{}: nothing mounted there", path.display());
        return ExitCode::FAILURE;
    };

    println!("{}", drive.mount_point.display());

    // A filesystem no player reads does not end the report. What is written to
    // the drive has to be put right too, and finding that out only after
    // reformatting means doing the work twice. The window has always said both.
    let refused = if let Some(filesystem) = drive.filesystem {
        report_readers(filesystem, &players)
    } else {
        println!("  {} — no player reads this", drive.reported_as);
        players.len()
    };

    let clean = report_contents(&drive.mount_point, &players, ffprobe);

    if refused == 0 && clean {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Which of `players` read this filesystem, returning how many will not.
fn report_readers(filesystem: FileSystem, players: &[&'static DeviceProfile]) -> usize {
    let name = filesystem_name(filesystem);
    println!("  {name}\n");

    let split = usb::readers_of(filesystem, DEVICES);
    let asked_about = |id: &str| players.iter().any(|player| player.id == id);

    let readable: Vec<_> = split
        .readable
        .iter()
        .filter(|player| asked_about(player.id))
        .map(|player| player.display_name)
        .collect();
    if !readable.is_empty() {
        println!("  reads it       {}", readable.join(", "));
    }

    let mut refused = 0usize;
    for (player, support) in &split.unreadable {
        if !asked_about(player.id) {
            continue;
        }
        refused += 1;

        let verdict = match support {
            Support::Conflicting => format!("sources disagree about {name}"),
            Support::Unknown => format!("{name} is undocumented for this player"),
            _ => format!("does not read {name}"),
        };
        println!("  {:<14} {verdict}", player.display_name);
    }

    refused
}

/// How many offending paths to name before summarising the rest.
///
/// A stick that is wrong is usually wrong in one place repeated many times, so
/// the first few name the problem and the rest would only bury it.
const NAMED_AT_MOST: usize = 10;

/// Report what is on the drive, returning whether every player would take it.
///
/// The walk itself opens nothing and comes back at once. Reading the tracks is
/// one ffprobe per file, so it runs on the pool and shows a count while it does.
fn report_contents(
    root: &Path,
    players: &[&'static DeviceProfile],
    ffprobe: Option<&Path>,
) -> bool {
    let Some(limits) = scan::Limits::strictest_of(players) else {
        return true;
    };

    let contents = scan::walk(root, limits);
    let tracks = contents.tracks.len();

    println!(
        "\n  {tracks} {}, {} {}, {} deep",
        plural(tracks, "track"),
        contents.folders,
        plural(contents.folders, "folder"),
        contents.deepest,
    );

    if contents.other_files > 0 {
        println!(
            "  {} {} no player will list",
            contents.other_files,
            plural(contents.other_files, "other file"),
        );
    }

    // Every count above is a count of what the walk reached, so anything that
    // leaves a hole in the tree decides this before a single file is read.
    let clean = !contents.has_gaps();

    if !contents.unreadable.is_empty() {
        println!("\n  could not be read, so nothing inside it was checked");
        list(
            contents
                .unreadable
                .iter()
                .map(|folder| format!("    {}", relative_to(root, folder))),
        );
    }

    if !contents.unreachable.is_empty() {
        println!(
            "\n  past {} folders deep, so the browser never reaches it",
            limits.folder_depth
        );
        list(
            contents
                .unreachable
                .iter()
                .map(|folder| format!("    {}", relative_to(root, folder))),
        );
    }

    for crowded in &contents.crowded {
        println!(
            "\n  {} holds {} entries, and the browser lists {}",
            relative_to(root, &crowded.folder),
            crowded.entries,
            limits.entries_per_folder.unwrap_or_default(),
        );
    }

    let Some(ffprobe) = ffprobe else {
        return clean;
    };

    let progress = Progress::new(tracks);
    let verdicts = scan::judge(
        &contents.tracks,
        ffprobe,
        players,
        parallel::default_concurrency(),
        &|_| progress.advance(),
    );
    progress.clear();

    let failing: Vec<_> = verdicts.iter().filter(|v| !v.plays()).collect();

    // Said either way round. A drive with one bad track is mostly good news, and
    // reporting only the failure leaves it to be worked out by subtraction.
    //
    // The count is of tracks the walk reached, which on a drive with a hole in
    // it is not the same as the tracks on the drive. Saying so plainly is the
    // difference between a promise and a report: "all 12 play" reads as a drive
    // that is ready even when the folders named above hold another hundred.
    if tracks > 0 {
        // "found" rather than a claim about the drive. A folder the browser
        // never reaches, one it cuts short, and one that would not open all
        // leave tracks outside this number, and only one of the three was ever
        // walked — so no single word covers what is missing except this one.
        let found = if clean { "" } else { " found" };
        println!(
            "\n  {} of the {tracks} {}{found} will play on every player named",
            tracks - failing.len(),
            plural(tracks, "track"),
        );
    }

    if failing.is_empty() {
        return clean;
    }

    println!(
        "\n  {} {} at least one player will not take",
        failing.len(),
        plural(failing.len(), "track"),
    );
    list(failing.iter().map(|verdict| {
        let why = match &verdict.error {
            Some(error) => first_line(error),
            None => verdict
                .refused_by
                .iter()
                .map(|refusal| {
                    let reasons: Vec<_> =
                        refusal.issues.iter().copied().map(describe_issue).collect();
                    format!("{}: {}", refusal.player.display_name, reasons.join("; "))
                })
                .collect::<Vec<_>>()
                .join(", "),
        };
        format!("    {:<38} {why}", relative_to(root, &verdict.path))
    }));

    false
}

/// Print at most [`NAMED_AT_MOST`] lines, then say how many were left out.
///
/// Silently stopping would read as "that is all of them", which is the one thing
/// a report of what is wrong must never imply.
fn list(lines: impl ExactSizeIterator<Item = String>) {
    let total = lines.len();

    for line in lines.take(NAMED_AT_MOST) {
        println!("{line}");
    }

    if let Some(rest) = total.checked_sub(NAMED_AT_MOST).filter(|rest| *rest > 0) {
        println!("    and {rest} more");
    }
}

/// One track to a line, so a column of them stays a column.
///
/// ffprobe's complaint runs to several lines and carries the address it was
/// loaded at, none of which says anything about the track. The first line does.
fn first_line(message: &str) -> String {
    let line = message.lines().next().unwrap_or(message).trim();

    // "could not read the file: [mp3 @ 0x…] Failed to find…" — ffprobe naming the
    // decoder it loaded and the address it landed at, between two useful parts.
    // Matched on the address, so a bracket that means something else survives.
    let tagged = line
        .find(": [")
        .and_then(|start| line[start..].find("] ").map(|end| (start, start + end)))
        .filter(|(start, end)| line[*start..*end].contains(" @ 0x"));

    match tagged {
        Some((start, end)) => format!("{}: {}", &line[..start], &line[end + 2..]),
        None => line.to_owned(),
    }
}

/// Paths shown as they sit on the drive, since the mount point is on the line
/// above and repeating it on every row buries the part that differs.
fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        noun.to_owned()
    } else {
        format!("{noun}s")
    }
}

fn filesystem_name(filesystem: FileSystem) -> &'static str {
    match filesystem {
        FileSystem::Fat16 => "FAT16",
        FileSystem::Fat32 => "FAT32",
        FileSystem::ExFat => "exFAT",
        FileSystem::HfsPlus => "HFS+",
        FileSystem::Ntfs => "NTFS",
    }
}

/// One line per file, written under a held lock so parallel workers cannot
/// interleave halfway through a line.
fn report_one(
    finished: usize,
    total: usize,
    job: &convert::Job,
    root: Option<&Path>,
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
                landed_at(&job.output, root)
            );
        }
        Err(error) => {
            let mut err = std::io::stderr().lock();
            let _ = writeln!(err, "[{finished}/{total}] {}: {error}", name.display());
        }
    }
}

/// Where a result went, said from the folder the results are rooted at.
///
/// The whole library path on every line would be unreadable, and the enclosing
/// folder alone is worse than that: a track nested five deep comes out nested
/// five deep, and naming only the folder it landed in leaves out the part that
/// says where to start looking.
fn landed_at(output: &Path, root: Option<&Path>) -> String {
    if let Some(under) = root.and_then(|root| output.strip_prefix(root).ok()) {
        return under.display().to_string();
    }

    let name = output.file_name().unwrap_or(output.as_os_str());
    match output.parent().and_then(Path::file_name) {
        Some(folder) => format!("{}/{}", folder.display(), name.display()),
        None => name.display().to_string(),
    }
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

/// The zsh glob for the same set. `(#i)` makes it case-insensitive, so a `.WAV`
/// ripped years ago still shows up.
fn audio_glob() -> String {
    format!("(#i)*.({})", files::AUDIO_EXTENSIONS.join("|"))
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

    let found = files::collect(files, PreviousOutput::Include);
    report_unreadable(&found.unreadable);

    let inputs = found.files;
    if inputs.is_empty() {
        eprintln!("no audio files among the paths given");
        return ExitCode::FAILURE;
    }

    let progress = Progress::new(inputs.len());

    // Read every file first, then report. Probing is one process per file and
    // parallelises; the report has to stay in the order the files were given.
    let specs = parallel::map(
        &inputs,
        parallel::default_concurrency(),
        &|_, file| probe::run(ffprobe, file),
        &|_, _| progress.advance(),
    );

    progress.clear();

    // Nothing here can be called clear while part of the tree went unread.
    let mut all_clear = found.unreadable.is_empty();
    let mut rejected_count = 0usize;

    for (file, outcome) in inputs.iter().zip(specs) {
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
    /// The count and the line it is written on, under one lock.
    ///
    /// Counted with an atomic and printed afterwards, two workers can take
    /// their numbers in order and reach the terminal in the other one — the
    /// counter is then seen going to 2 of 2 and back to 1 of 2.
    done: Mutex<usize>,
}

impl Progress {
    fn new(total: usize) -> Self {
        use std::io::IsTerminal;
        Self {
            total,
            to_a_terminal: std::io::stderr().is_terminal(),
            done: Mutex::new(0),
        }
    }

    /// One more finished, shown as it lands.
    fn advance(&self) {
        use std::io::Write;
        let mut done = self.done.lock().unwrap_or_else(PoisonError::into_inner);
        *done += 1;

        if !self.to_a_terminal {
            return;
        }
        let mut err = std::io::stderr().lock();
        let _ = write!(err, "{}", progress_line(*done, self.total));
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

    // Said here and nowhere else in the report, because every other line
    // answers "will it play" and this one answers "is it worth playing". A
    // player takes it happily; a room hears the encoder.
    if plan::sounds_thin(spec) {
        println!(
            "  thin           under {} kbps, and converting cannot put it back",
            plan::THIN_BITRATE_KBPS
        );
    }

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

    /// One drive can hold thousands of unreadable files, and ffprobe answers
    /// each with several lines and the address it happened to load at. Left
    /// alone that turns a column of tracks into an unreadable wall.
    #[test]
    fn a_failure_reads_as_one_line_about_the_track() {
        let raw = "ffprobe could not read the file: [mp3 @ 0xaacc38000] Failed to \
                   find two consecutive MPEG audio frames.\n\
                   /Volumes/DJ/track.mp3: Invalid data found when processing input";

        assert_eq!(
            first_line(raw),
            "ffprobe could not read the file: Failed to find two consecutive MPEG audio frames."
        );
    }

    /// Most failures carry no decoder tag, and must come through untouched.
    #[test]
    fn a_message_without_ffprobes_noise_is_left_alone() {
        assert_eq!(first_line("no such file"), "no such file");
        assert_eq!(first_line("gave up: [1] of 2"), "gave up: [1] of 2");
    }
}
