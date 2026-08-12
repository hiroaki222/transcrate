# Bundled ffmpeg

Released builds carry their own ffmpeg and ffprobe, because the people the
window exists for are the ones who will not install one. The workflow in
`.github/workflows/release.yml` puts them here, named with the Rust target
triple as Tauri expects:

    ffmpeg-aarch64-apple-darwin
    ffprobe-aarch64-apple-darwin
    ffmpeg-x86_64-pc-windows-msvc.exe

They are not committed. They are large, they change on their own schedule,
and a binary in a source tree is a binary nobody can audit against its source.

## Licence

Only LGPL builds go in here. This program is MIT or Apache-2.0 and runs ffmpeg
as a separate process, but shipping a GPL build inside the same bundle would
bring GPL obligations to the thing being shipped. An LGPL build covers every
format Transcrate writes: MP3 through libmp3lame, AAC through ffmpeg's own
encoder, and FLAC, ALAC and PCM natively.

Windows and Linux take BtbN's published LGPL builds. No LGPL build is published
for macOS, so the workflow compiles one, with the GPL-only components left out.

## Running from a checkout

There is nothing here, and there does not need to be. With no sidecar beside
the executable the app falls back to whatever `ffmpeg` is on the PATH.
