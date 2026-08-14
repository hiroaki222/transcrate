# Transcrate

[日本語](README.ja.md)

Convert tracks for your USB stick, and know they will play before you get to the
club.

[**Download**](https://github.com/hiroaki222/transcrate/releases/latest): a
`.dmg` for Apple silicon, an `.exe` for Windows. Both carry their own ffmpeg, so
there is nothing else to install.

![The app, with a folder of tracks and what each one will play on](docs/images/convert.png)

Each track shows ten lights, one per player, in a fixed order. Failing lights
are hatched as well as red, so reading them does not depend on colour. Under a
target that makes no promise about playback, a second row shows what the
conversion would leave. The two targets that do promise it get no second row.

A command line does the same work, and both read the same compatibility table,
taken from the manufacturers' manuals.

## Why this exists

DJ gear disagrees with itself, and not in ways you can guess:

- **A CDJ-2000NXS2 from 2016 plays 96 kHz FLAC. An XDJ-AN from 2026 stops at
  48 kHz.** Release year does not predict the limits.
- **That same NXS2 cannot read an exFAT stick.** Everything from 2020 on can,
  except the XDJ-RX3, which reads exFAT and refuses 96 kHz. The limits cross,
  so you cannot rank the players on one scale.
- **`.m4a` holds either AAC or ALAC.** Some players take only AAC and throw
  `E-8305` at the other. The extension gives you no warning.

Get one of these wrong and you find out in the booth.

## What plays where

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

Every figure is from a manufacturer's manual, with the document number recorded
in [docs/device-compatibility.md](docs/device-compatibility.md).

The official sources contradict each other about the XDJ-XZ and exFAT, and the
table says so. The app takes the stricter reading and shows a plain no, because
you cannot settle that in a booth.

## The app

[Download it](https://github.com/hiroaki222/transcrate/releases/latest), open
it, and drag tracks or a folder onto it.

Neither build is signed with a paid certificate, so both systems warn the first
time it opens. Apple's certificate costs $99 a year, which is hard to justify
before anyone is using this. Allowing it once is enough. Apple documents the
macOS side:
[Open a Mac app from an unknown developer][unsigned-mac]. On Windows,
SmartScreen asks for **More info → Run anyway**.

Three screens:

- **CONVERT**: each row says what the file is, what it would become, and ten
  lights: one per player, green where it plays and hatched red where it will
  not. Only the rows that want something are labelled, so you can scan a list
  of forty instead of reading every line. A source already under 192 kbps
  carries a caution of its own: it plays, and converting cannot put back what
  its first encoder threw away.

  ![Each track with its verdict now and after converting](docs/images/convert.png)

- **USB CHECK**: pick from the drives that are plugged in. It reports the
  filesystem, then reads every track on the drive and names the ones a player
  will refuse. It also measures the tree: the players stop at eight folder
  levels and list ten thousand entries per folder, and past either of those a
  drive mounts, the tracks are there, and the browser shows nothing. It names
  any folder it could not read, and while anything is missing the summary does
  not call the drive clean. Read-only, with no format button.

  ![A drive checked against every player](docs/images/usb-check.png)

- **DEVICES**: the table above, release year beside each player.

The app uses whatever language the machine is set to, Japanese or English, and
you can pin either one.

## The command line

This one expects ffmpeg on your PATH; only the app brings its own.

On macOS and Linux, Homebrew is the shorter way in, because it brings ffmpeg
with it:

```sh
brew install hiroaki222/tap/transcrate
```

Otherwise take the archive for your platform from the release. It holds the
binary and nothing that installs it. The names carry the target:

| | |
|---|---|
| Apple silicon | `transcrate-<version>-aarch64-apple-darwin.tar.gz` |
| Linux x86-64 | `transcrate-<version>-x86_64-unknown-linux-gnu.tar.gz` |
| Windows x86-64 | `transcrate-<version>-x86_64-pc-windows-msvc.zip` |

```sh
tar -xzf transcrate-*-aarch64-apple-darwin.tar.gz
sudo mv transcrate /usr/local/bin/
```

macOS quarantines a downloaded binary and refuses the first run without a word
about why. Clear it once:

```sh
xattr -d com.apple.quarantine /usr/local/bin/transcrate
```

Windows: unzip it and put `transcrate.exe` somewhere on your PATH. Either way,
ffmpeg has to be there too: `brew install ffmpeg`, or whatever your system
uses.

```sh
transcrate convert ~/Music
```

Three ways to say "all of them":

```sh
transcrate convert ~/Music              # the folder, subfolders and all
transcrate convert *                    # whatever the shell expands, audio only
transcrate convert a.wav b.flac         # named one by one
```

A folder sweep and a glob both keep only the audio, so artwork and playlists
never show up as failures. It skips a previous run's `_transcrate` folder too,
so converting twice does not re-encode what came out the first time.

Transcrate tries a path named on its own whatever its extension: someone who
typed a single filename meant that file, and ffprobe judges it better than the
extension does.

Options go on either side of the files, so `convert -p lossless track.wav` and
`convert track.wav -p lossless` are the same command. Track names full of `&`
and brackets are easier through the folder form, or by letting tab completion
escape them for you.

```
[1/3] thin.mp3 -> _transcrate/thin.mp3  (tags rewritten, audio untouched)
[2/3] deep.flac -> _transcrate/2024/Live/deep.mp3  (encoded)
[3/3] opener.wav -> _transcrate/opener.mp3  (encoded)
```

A folder handed over whole keeps its shape: one `_transcrate` beside it, and
each track written at the depth it was read from. Files named one by one have
no shape to keep, so each result sits beside its own source.

The source is never written to. Two inputs that would produce the same output
are both refused: `mix.wav` and `mix.flac` in one folder both ask for
`mix.mp3`, and neither of them gets to overwrite the other.

Transcrate copies a file that already matches the target format, which is
faster and spares a lossy original a second pass through an encoder. A lossy
source is never re-encoded above the bitrate it arrived with, either: 128 kbps
asked for `cdj-safe` comes out at 128 kbps, because the same music through a
second encoder sounds worse than it did and takes two and a half times the
space.

Files convert in parallel, one per core, and each line appears as that file
lands. Fourteen 60-second 96 kHz FLACs took 2.96 s one at a time and 0.56 s
across 14 cores here. `-j N` caps the number of jobs.

Three profiles, chosen with `-p`:

| Profile | Output | For |
|---|---|---|
| `cdj-safe` (default) | MP3 320 kbps, 44.1 kHz | Plays on every player in the table |
| `lossless` | AIFF, up to 48 kHz / 24-bit | Lossless and still playable everywhere |
| `archive` | FLAC, source rate and depth | The copy you keep at home |

Or name a format directly. That changes the container and nothing else, keeping
the source's rate and depth:

```sh
transcrate convert ~/Music/track.flac --to aiff
```

`mp3`, `aac`, `alac`, `flac`, `wav`, `aiff`. A profile carries limits with it
and a format does not, so a 96 kHz source stays at 96 kHz. Run `check` on the
result if it is going to a gig.

Reducing bit depth adds dither. Resampling does not, because that is not what
dither is for.

### Checking files

```sh
transcrate check ~/Music/track.flac
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

Narrow it to the gear you are taking, and leave out everything that already
plays:

```sh
transcrate check ~/Music --failing -d cdj-3000,xdj-rr
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

A track can clear every player and still be worth knowing about:

```
Set B/thin.mp3
  MP3 44.1 kHz 128 kbps
  thin           under 192 kbps, and converting cannot put it back
  plays on       CDJ-3000X, CDJ-3000, CDJ-2000NXS2, XDJ-AZ, XDJ-AN, XDJ-XZ, …
```

A counter runs on stderr while it works, and only when stderr is a terminal, so
piping the report into a file or another program keeps it clean. It exits
non-zero if anything is rejected, so it can gate a script.

### Checking a drive

Named with no path, it lists what is plugged in, which saves you looking up a
mount point:

```sh
transcrate usb
```

```
DJ                   exFAT    /Volumes/DJ
KOMORI               FAT32    /Volumes/KOMORI
```

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

  2 tracks, 1 folder, 2 deep

  1 of the 2 tracks will play on every player named

  1 track at least one player will not take
    Set/02 Peak Time.m4a    XDJ-XZ: ALAC is not supported, XDJ-RX3: ALAC is …
```

Formatting a stick exFAT is what most tools do by default, and it locks out two
players that are still in plenty of booths. `-d` narrows it to the gear you are
plugging into, and it exits non-zero if any of those will not read the drive.

Below the filesystem it reads every track on the drive, which is one ffprobe
each and the slow half. `--no-tracks` stops after the filesystem. It also
measures the tree, because the players stop at eight folder levels and list ten
thousand entries per folder. Past either of those the drive mounts, the tracks
are there, and the browser shows nothing.

A folder the walk could not open gets named in the report, whether a permission
was refused or a stick was pulled part way through. The count that follows then
says it covers what was found, not the whole drive. A run that could not read
everything exits non-zero.

**Read-only.** Nothing here writes to a drive, formats one or moves a file. A
tool you run on your own set should not be able to damage it.

### Tags and artwork

Everything the source carried comes across, except `lyrics-eng`. You do not
read lyrics off a CDJ, and that field is where rippers leave their advertising.
Title, artist, album, genre, key and BPM are what the browser is for, so they
stay.

The comment stays too. Shops fill it with advertising and a CDJ shows it in the
browser next to the title, which is an argument for clearing it. It is also
where DJs keep their own cue notes and Camelot keys, and those cannot be got
back. `--clear-comment` empties it when you want that.

Embedded artwork rides along, labelled the way rekordbox and the CDJ browser
expect to find it. `--no-artwork` drops it instead.

Two things the muxers get wrong by default:

- **MP3 and AIFF are written as ID3v2.3**, not ffmpeg's default of 2.4. Players
  are more consistent with 2.3.
- **The AIFF muxer writes no ID3 chunk unless asked**, and the artwork goes with
  it. AIFF's own chunks still carry the title and artist, so the loss shows up
  as a missing sleeve instead of an untagged file. Transcrate sets that flag.

To fix tags without touching the audio:

```sh
transcrate retag ~/Music
```

```
[1/3] track.aiff -> _transcrate/track.aiff  (tags rewritten, audio untouched)
[2/3] already.mp3 -> _transcrate/already.mp3  (tags rewritten, audio untouched)
[3/3] track.flac -> _transcrate/track.flac  (tags rewritten, audio untouched)
```

Every file comes out in the format it went in as, so a folder holding MP3 next
to AIFF takes one command instead of one per extension. Transcrate copies the
audio stream across untouched, so a lossy file loses nothing to a change of
text and no time goes into re-encoding audio that was already correct.

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

## Build from source

Needs Rust 1.88 or newer, and ffmpeg on your PATH. A checkout carries no bundled
copy and falls back to whatever `ffmpeg` it finds, which is also what anyone
keeping their own build would want.

```sh
git clone https://github.com/hiroaki222/transcrate
cd transcrate
cargo run -p transcrate-cli -- devices
```

To put the command line on your PATH:

```sh
cargo install --path crates/transcrate-cli --locked
transcrate completions zsh > ~/.zfunc/_transcrate
```

Run both again after pulling. The binary and the completion script are
generated separately, so a stale install answers `unrecognized subcommand` for
a command the source has, and a stale completion offers flags that no longer
exist.

The app needs [Bun](https://bun.sh) as well:

```sh
cd gui
bun install
bun run tauri dev
```

`bun run tauri build` produces a `.dmg` on macOS and an `.exe` installer on
Windows, without the bundled ffmpeg that a release carries.

## Contributing

Run what CI runs:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features

cd gui && bun install --frozen-lockfile && bun run build
```

The last one is a separate CI job and the easiest to forget. It typechecks the
window, so a broken import passes everything above it and fails on the way in.

The integration tests run real conversions and refuse to skip when ffmpeg is
missing on CI, because they are the only thing checking that the argument lists
put in front of the encoder work.

## Next

- `--json`, so other programs can act on the verdicts

Apple silicon only on macOS. The last Intel Mac shipped in 2020, and supporting
one costs a second ffmpeg build and a universal bundle.

[unsigned-mac]: https://support.apple.com/guide/mac-help/open-a-mac-app-from-an-unknown-developer-mh40616/mac

## Licence

[MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), whichever you prefer.

ffmpeg runs as a separate process and is not linked into this program.

Released builds of the app carry an **LGPL** ffmpeg beside the executable, never
a GPL one: this program is MIT or Apache-2.0, and a GPL binary in the same
bundle would carry GPL obligations into it. An LGPL build covers every format
written here: MP3 through libmp3lame, AAC through ffmpeg's own encoder, and
FLAC, ALAC and PCM natively. Nobody publishes an LGPL build for macOS, and the
one published for Windows is a full build, 115 MB a binary against the 4 MB a
trimmed one comes to, carried inside every download. So
[the release workflow compiles both](.github/scripts/build-ffmpeg.sh) from the
same list of formats, with the GPL-only components left out.
