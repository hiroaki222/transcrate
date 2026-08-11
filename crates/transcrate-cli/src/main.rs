use clap::{Parser, Subcommand};
use transcrate_core::{Codec, DEVICES, DeviceProfile, FileSystem, Support};

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
}

fn main() {
    match Cli::parse().command {
        Command::Devices => print_devices(),
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
        .map_or_else(|| "-".to_owned(), |hz| khz(*hz))
}

fn khz(hz: u32) -> String {
    if hz.is_multiple_of(1_000) {
        format!("{}k", hz / 1_000)
    } else {
        format!("{:.1}k", f64::from(hz) / 1_000.0)
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
