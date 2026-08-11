# Transcrate

Audio transcoder for DJs, built on ffmpeg, that knows what your gear can
actually play.

## Why

Converting tracks for a USB stick looks solved until the gear disagrees with
itself:

- A CDJ-2000NXS2 from 2016 plays 96 kHz FLAC. An XDJ-AN from 2026 stops at
  48 kHz. Newer is not more capable.
- That same CDJ-2000NXS2 cannot read an exFAT stick, which every player from
  2020 onward can. The XDJ-RX3 reads exFAT but rejects 96 kHz. The two limits
  cross, so the players cannot be ranked on one scale.
- `.m4a` holds either AAC or ALAC. Several players accept only AAC and answer
  the other with error `E-8305`, and the file extension warns you of nothing.

Transcrate keeps the published limits of each player in one table, checks your
output against the machines you are actually going to plug into, and tells you
before you are standing in the booth.

## What works today

The compatibility table and the `devices` command. Conversion is not built yet.

## Requirements

- Rust 1.85 or newer (edition 2024)

ffmpeg is not required yet. Once conversion lands it will be invoked as a
separate process, using a system installation when one is present and a bundled
LGPL build otherwise.

## Build and run

```sh
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

Tests and lints:

```sh
cargo test
cargo clippy --all-targets
```

## Compatibility data

Every figure in the table comes from the manufacturer's own operating
instructions, with the document number recorded in
[docs/device-compatibility.md](docs/device-compatibility.md). Where the official
sources contradict each other — the XDJ-XZ and exFAT — the table records the
disagreement rather than picking a side.

## Planned

- Convert between WAV, FLAC, AIFF, M4A and MP3 with DJ-appropriate defaults
- Warnings per player, from the table above
- Read-only USB diagnostics, which never write to or format a drive
- Per-field metadata control: keep, clear or overwrite
- Profiles shared between the command line and the GUI
- A Tauri GUI for macOS and Windows, over the same core

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.
