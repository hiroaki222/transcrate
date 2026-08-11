# Transcrate

[日本語](README.ja.md)

Convert tracks for your USB stick, and know they will play before you get to the
club.

Transcrate converts audio with ffmpeg and checks the result against what CDJs
and XDJs actually accept: codecs, sample rates, bit depths and filesystems,
taken from the manufacturers' manuals.

**Status: early, but it converts.** Parallel jobs, progress reporting and
metadata control are next.

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

Convert a folder. Needs ffmpeg on your PATH:

```sh
cargo run -p transcrate-cli -- convert ~/Music
```

Three ways to say "all of them":

```sh
transcrate convert ~/Music              # the folder, subfolders and all
transcrate convert *                    # whatever the shell expands, audio only
transcrate convert a.wav b.flac         # named one by one
```

A folder sweep and a glob both keep only the audio, so artwork and playlists are
skipped rather than reported as failures. A previous run's `_transcrate` folder
is skipped too, and converting twice does not re-encode what came out the first
time.

One path named on its own is always attempted, whatever its extension: someone
who typed a single filename meant that file, and ffprobe judges it better than
the extension does.

Options go on either side of the files, so `convert -p lossless track.wav` and
`convert track.wav -p lossless` are the same command. Track names full of `&`
and brackets are easier through the folder form, or by letting tab completion
escape them for you.

```
~/Music/track.flac
  FLAC 96 kHz 24-bit -> MP3 44.1 kHz 320 kbps  (encoded)
  ~/Music/_transcrate/track.mp3
~/Music/already-fine.mp3
  MP3 44.1 kHz 320 kbps -> MP3 44.1 kHz 320 kbps  (copied unchanged)
  ~/Music/_transcrate/already-fine.mp3
```

Results land in a `_transcrate` folder beside each input, and the source is
never written to. Anything already in the target format is copied rather than
re-encoded, which is both faster and kinder to a lossy original.

Files convert in parallel, one per core, and each line appears as that file
lands. Fourteen 60-second 96 kHz FLACs down to MP3 took 2.96 s one at a time
and 0.56 s across 14 cores here — the same CPU time, five times less waiting.
`-j N` caps the number of jobs if you want the machine back.

Three profiles, chosen with `-p`:

| Profile | Output | For |
|---|---|---|
| `cdj-safe` (default) | MP3 320 kbps, 44.1 kHz | Plays on every player in the table |
| `lossless` | AIFF, up to 48 kHz / 24-bit | Lossless and still playable everywhere |
| `archive` | FLAC, source rate and depth | The copy you keep, not the one you play |

Or name a format directly. That changes the container and nothing else, keeping
the source's rate and depth:

```sh
cargo run -p transcrate-cli -- convert ~/Music/track.flac --to aiff
```

`mp3`, `aac`, `alac`, `flac`, `wav`, `aiff`. A profile carries limits with it
and a format does not, so a 96 kHz source stays at 96 kHz — run `check` on the
result if it is going to a gig.

Reducing bit depth adds dither automatically. Resampling does not, because
that is not what dither is for.

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

### Installing it on your PATH

```sh
cargo install --path crates/transcrate-cli --locked
transcrate completions zsh > ~/.zfunc/_transcrate
```

Run both again after pulling. The binary and the completion script are
generated separately, so a stale install answers `unrecognized subcommand` for
a command the source has, and a stale completion offers flags that no longer
exist.

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
