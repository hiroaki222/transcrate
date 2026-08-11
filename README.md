# Transcrate

[日本語](README.ja.md)

Convert tracks for your USB stick, and know they will play before you get to the
club.

Transcrate converts audio with ffmpeg and checks the result against what CDJs
and XDJs actually accept: codecs, sample rates, bit depths and filesystems,
taken from the manufacturers' manuals.

**Status: early.** `devices` and `check` work. Conversion is not built yet.

## Build

Needs Rust 1.88 or newer.

```sh
git clone https://github.com/hiroaki222/transcrate
cd transcrate
cargo run -p transcrate-cli -- devices
```

```
DEVICE         YEAR     MP3   AAC    WAV   AIFF   FLAC   ALAC  EXFAT
XDJ-AN         2026     48k   48k    48k    48k    48k    48k  yes
CDJ-3000X      2025     48k   48k    96k    96k    96k    96k  yes
XDJ-AZ         2025     48k   48k    96k    96k    96k    96k  yes
OMNIS-DUO      2024     48k   48k    48k    48k    48k    48k  yes
OPUS-QUAD      2023     48k   48k    96k    96k    96k    96k  yes
XDJ-RX3        2021     48k   48k    48k    48k    48k      -  yes
CDJ-3000       2020     48k   48k    96k    96k    96k    96k  yes
XDJ-XZ         2019     48k   48k    48k    48k    48k      -  sources disagree
XDJ-RR         2018     48k   48k    48k    48k      -      -  no
CDJ-2000NXS2   2016     48k   48k    96k    96k    96k    96k  no
```

Ask what a file will play on. This one needs `ffprobe` on your PATH, which comes
with ffmpeg:

```sh
cargo run -p transcrate-cli -- check ~/Music/track.flac
```

```
~/Music/track.flac
  FLAC 96 kHz 24-bit
  plays on       CDJ-3000X, XDJ-AZ, OPUS-QUAD, CDJ-3000, CDJ-2000NXS2
  XDJ-AN         96 kHz is not supported for FLAC
  OMNIS-DUO      96 kHz is not supported for FLAC
  XDJ-RX3        96 kHz is not supported for FLAC
  XDJ-XZ         96 kHz is not supported for FLAC
  XDJ-RR         FLAC is not supported
```

Narrow it to the gear you are actually taking:

```sh
cargo run -p transcrate-cli -- check ~/Music/*.flac --device cdj-3000,xdj-rr
```

It exits non-zero if anything is rejected, so it can gate a script.

### Shell completion

```sh
mkdir -p ~/.zfunc
transcrate completions zsh > ~/.zfunc/_transcrate
```

Then, in `~/.zshrc`:

```sh
fpath=("$HOME/.zfunc" $fpath)
autoload -Uz compinit && compinit
```

`bash`, `fish`, `powershell` and `elvish` work too. Player ids complete as well,
so `--device <TAB>` lists all ten.

Under zsh, file arguments offer audio files and directories only, so the folder
of artwork and PDFs sitting next to your tracks stays out of the way.

Tests and lints, the same three CI runs:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Why

DJ gear disagrees with itself, and not in ways you can guess:

- **A CDJ-2000NXS2 from 2016 plays 96 kHz FLAC. An XDJ-AN from 2026 stops at
  48 kHz.** Newer is not better.
- **That same NXS2 cannot read an exFAT stick.** Everything from 2020 on can —
  except the XDJ-RX3, which reads exFAT and refuses 96 kHz. The limits cross,
  so you cannot rank the players on one scale.
- **`.m4a` holds either AAC or ALAC.** Some players take only AAC and throw
  `E-8305` at the other. The extension gives you no warning.

Get one of these wrong and you find out in the booth.

## Where the numbers come from

Every figure is from a manufacturer's manual, with the document number recorded
in [docs/device-compatibility.md](docs/device-compatibility.md).

Where the official sources contradict each other — the XDJ-XZ and exFAT — the
table says so rather than picking a side.

## Roadmap

- Convert between WAV, FLAC, AIFF, M4A and MP3
- Warn per player, from the table above
- Check a USB stick. Read-only: it never writes to your drive
- Keep, clear or overwrite metadata field by field
- Profiles shared by the CLI and the GUI
- A GUI for macOS and Windows, on the same core

## Releases

None yet. When they start:

- **CLI** — a Homebrew tap and prebuilt binaries.
- **GUI** — `.dmg` and `.msi`, unsigned. An Apple developer certificate costs
  $99 a year, which is hard to justify before anyone is using this. macOS blocks
  an unsigned app the first time it is opened, and Apple documents the way
  through: [Open a Mac app from an unknown developer][unsigned-mac]. You do it
  once. On Windows, SmartScreen asks for **More info → Run anyway**.

[unsigned-mac]: https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac

## Licence

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), whichever you prefer.

ffmpeg runs as a separate process and is not linked into this program. Released
builds bundle an LGPL build of it, and prefer a system install when there is
one.
