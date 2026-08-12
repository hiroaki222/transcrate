# Transcrate

[日本語](README.ja.md)

Convert tracks for your USB stick, and know they will play before you get to the
club.

Transcrate converts audio with ffmpeg and checks the result against what CDJs
and XDJs actually accept: codecs, sample rates, bit depths and filesystems,
taken from the manufacturers' manuals.

**Status: it works, and there is nothing to download yet.** Conversion, the
per-player verdict, the drive check and the window are all in place; releases
are not.

## The window

For anyone who would rather not open a terminal. Same core, same table, same
answers.

```sh
cd gui
bun install
bun run tauri dev
```

Needs [Bun](https://bun.sh) and ffmpeg on your PATH. `bun run tauri build`
produces a `.app` on macOS and an `.msi` on Windows.

Three screens:

- **CONVERT** — drop tracks or a folder on the window. Each row says what the
  file is, what it would become, and carries ten lamps: one per player, green
  where it plays and hatched red where it will not. A second row of lamps shows
  the verdict after conversion, so a red row can be seen turning green before
  anything is committed to.
- **USB CHECK** — point it at a drive and see which players will read it.
  Read-only, and there is no format button.
- **DEVICES** — the compatibility table itself, release year beside each player.

The interface follows whatever language the machine is set to, Japanese or
English, and can be pinned to either.

Where official sources contradict each other the window takes the stricter
reading, so the XDJ-XZ's disputed exFAT support shows as a plain no. A
contradiction is not something anyone can settle in a booth.

## Command line

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

### Tags and artwork

Everything the source carried comes across, except `lyrics-eng`. Nobody reads
lyrics off a CDJ, and it is where rippers leave their advertising. Title,
artist, album, genre, key and BPM are what the browser is for, so they stay.

The comment stays too. Shops fill it with advertising and a CDJ shows it in the
browser next to the title, which is an argument for clearing it — but it is
also where DJs keep their own cue notes and Camelot keys, and those cannot be
got back. `--clear-comment` empties it when you want that.

### Tidying tags without converting

```sh
transcrate retag ~/Music
```

```
[1/3] track.aiff -> _transcrate/track.aiff  (tags rewritten, audio untouched)
[2/3] already.mp3 -> _transcrate/already.mp3  (tags rewritten, audio untouched)
[3/3] track.flac -> _transcrate/track.flac  (tags rewritten, audio untouched)
```

Every file comes out in the format it went in as, so a folder holding MP3 next
to AIFF takes one command rather than one per extension. The audio stream is
copied across untouched: a lossy file loses nothing to a change of text, and
nothing is spent re-encoding audio that was already correct.

`--no-artwork` and `--clear-comment` mean the same here as on `convert`:

```sh
transcrate retag ~/Music --no-artwork                 # drop every sleeve
transcrate retag ~/Music --no-artwork --clear-comment  # sleeves out, comments too
```

Embedded artwork rides along, labelled the way rekordbox and the CDJ browser
expect to find it. `--no-artwork` drops it instead.

Two details that are easy to lose:

- **MP3 and AIFF are written as ID3v2.3**, not ffmpeg's default of 2.4. Players
  are more consistent with 2.3.
- **The AIFF muxer writes no ID3 chunk unless asked**, and the artwork goes with
  it. AIFF's own chunks still carry the title and artist, so the loss shows up
  as a missing sleeve rather than as an untagged file. That flag is set here.

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

`--failing` leaves out everything that already plays, so a folder prints only
what needs doing:

```sh
transcrate check ~/Music --failing -d xdj-rr
```

```
./float32.wav
  WAV 48 kHz 32-bit
  XDJ-RR         32-bit is not supported for WAV

./hires.flac
  FLAC 96 kHz 24-bit
  XDJ-RR         FLAC is not supported

2 of 6 rejected
```

Failing means *any* of the named players rejects it, not all of them: a track
that plays on nine out of ten is still the one that stops the set.

A counter runs on stderr while it works, and only when stderr is a terminal, so
piping the report into a file or another program keeps it clean.

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

## Checking a drive

```sh
transcrate usb /Volumes/DJ
```

```
/Volumes/DJ
  exFAT

  reads it       CDJ-3000X, CDJ-3000, XDJ-AZ, XDJ-AN, XDJ-RX3, OMNIS-DUO, OPUS-QUAD
  CDJ-2000NXS2   does not read exFAT
  XDJ-XZ         sources disagree about exFAT
  XDJ-RR         does not read exFAT
```

Formatting a stick exFAT is the easy default and it locks out two players that
are still in plenty of booths. `-d` narrows it to the gear you are actually
plugging into, and it exits non-zero if any of those will not read the drive.

**Read-only.** Nothing here writes to a drive, formats one or moves a file. A
tool you point at your set on a Friday evening has no business being able to
damage it.

## Where the numbers come from

Every figure is from a manufacturer's manual, with the document number recorded
in [docs/device-compatibility.md](docs/device-compatibility.md).

Where the official sources contradict each other — the XDJ-XZ and exFAT — the
table says so rather than picking a side.

## Roadmap

Working:

- Convert between WAV, FLAC, AIFF, M4A and MP3, several at once
- A verdict per player, from the table above
- Check a USB stick. Read-only: it never writes to your drive
- Tags and artwork carried across, cleared or left alone
- A window for macOS and Windows, on the same core as the command line

Next:

- Bundle ffmpeg, so nothing has to be installed before the app will run
- Read a stick's contents, not only its filesystem
- `--json`, so other programs can act on the verdicts

## Releases

None cut yet, but tagging one builds and attaches:

- **The window** — a `.dmg` for Apple silicon and an `.exe` installer for
  Windows, each carrying its own ffmpeg so nothing has to be installed first.
- **The command line** — an archive per platform, holding one binary. This one
  expects ffmpeg on your PATH.

Apple silicon only on macOS. The last Intel Mac shipped in 2020, and building
for one costs a second ffmpeg and a universal bundle.

Both are unsigned. An Apple developer certificate costs $99 a year, which is
hard to justify before anyone is using this. macOS blocks an unsigned app the
first time it is opened, and Apple documents the way through: [Open a Mac app
from an unknown developer][unsigned-mac]. You do it once. On Windows,
SmartScreen asks for **More info → Run anyway**.

[unsigned-mac]: https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac

## Licence

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), whichever you prefer.

ffmpeg runs as a separate process and is not linked into this program.

Released builds of the window carry an **LGPL** ffmpeg beside the executable,
never a GPL one: this program is MIT or Apache-2.0, and a GPL binary in the
same bundle would carry GPL obligations into it. An LGPL build covers every
format written here — MP3 through libmp3lame, AAC through ffmpeg's own encoder,
and FLAC, ALAC and PCM natively. Windows takes BtbN's published LGPL build;
nobody publishes one for macOS, so
[the release workflow compiles it](.github/scripts/build-ffmpeg-macos.sh) with
the GPL-only components left out.

A checkout has no such copy and falls back to whatever `ffmpeg` is on your
PATH, which is also what anyone keeping their own build would want.
