use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::builder::PossibleValuesParser;
use clap::{CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::Shell;
use transcrate_core::{
    AudioSpec, Codec, DEVICES, DeviceProfile, FileSystem, Issue, Support, by_id, check, probe,
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
        Command::Completions { shell } => {
            write_completions(shell, &mut std::io::stdout());
            ExitCode::SUCCESS
        }
    }
}

fn write_completions(shell: Shell, out: &mut impl std::io::Write) {
    clap_complete::generate(shell, &mut Cli::command(), "transcrate", out);
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

    #[test]
    fn a_completion_script_names_the_subcommands() {
        let mut script = Vec::new();
        write_completions(clap_complete::Shell::Zsh, &mut script);
        let script = String::from_utf8(script).expect("utf8");

        assert!(script.contains("transcrate"));
        assert!(script.contains("check"));
        assert!(script.contains("devices"));
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
