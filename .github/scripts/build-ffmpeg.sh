#!/usr/bin/env bash
#
# Build a static, LGPL ffmpeg and ffprobe.
#
# Nobody publishes an LGPL macOS build. Every prebuilt one found so far is GPL,
# because they all enable x264 — and Transcrate is MIT or Apache-2.0, so a GPL
# binary inside the same bundle would carry GPL obligations into the thing
# being shipped.
#
# Leaving out --enable-gpl costs nothing here: MP3 comes from libmp3lame,
# which is LGPL, AAC from ffmpeg's own encoder, and FLAC, ALAC and PCM are
# native. Those are every format Transcrate writes.
#
# Windows has an LGPL build published, but it is a full one: 115 MB per binary
# against the 4 MB this produces, and all of it inside every download. The
# formats are the same on both systems, so the list below is the one that
# decides — kept here once, because two copies of it would drift.
#
# Usage: build-ffmpeg.sh <macos|windows> <output-directory>

set -euo pipefail

target="${1:?target: macos or windows}"
out="$(cd "$(dirname "${2:?output directory}")" && pwd)/$(basename "$2")"
mkdir -p "$out"

case "$target" in
  macos)
    host=()
    cross=()
    ldflags=""
    strip_with="strip"
    suffix=""
    cores="$(sysctl -n hw.ncpu)"
    ;;
  windows)
    # Cross-compiled from Linux. A Windows runner would need MSYS2 and a
    # toolchain build before it started; this needs one apt install.
    triple="x86_64-w64-mingw32"
    host=("--host=$triple")
    cross=(
      --enable-cross-compile
      "--cross-prefix=$triple-"
      --target-os=mingw32
      --arch=x86_64
    )
    # -static, or the toolchain's own runtime is left as a DLL dependency.
    # It would resolve on any machine with MSYS2 on its PATH — every runner
    # this could be tested on — and on no machine belonging to anyone who
    # downloads this.
    ldflags="-static"
    strip_with="$triple-strip"
    suffix=".exe"
    cores="$(nproc)"
    ;;
  *)
    echo "unknown target: $target (expected macos or windows)" >&2
    exit 1
    ;;
esac

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cd "$work"

# Pinned rather than tracking a moving target: a release should be able to
# rebuild the same binary a year from now.
LAME_VERSION="3.100"
FFMPEG_VERSION="7.1.1"

prefix="$work/prefix"
mkdir -p "$prefix"

echo "==> lame $LAME_VERSION for $target"
curl -fsSL -o lame.tar.gz \
  "https://downloads.sourceforge.net/project/lame/lame/$LAME_VERSION/lame-$LAME_VERSION.tar.gz"
tar -xzf lame.tar.gz
cd "lame-$LAME_VERSION"
# frontend/ builds the `lame` command line tool, which needs nothing we ship
# and fails on macOS over a missing termcap.
# ${a[@]:+...} rather than ${a[@]}: an empty array under `set -u` is an
# unbound variable on the bash macOS ships, which is bash 3.2.
./configure ${host[@]:+"${host[@]}"} --prefix="$prefix" \
  --disable-shared --enable-static --disable-frontend
make -j"$cores"
make install
cd "$work"

echo "==> ffmpeg $FFMPEG_VERSION for $target"
curl -fsSL -o ffmpeg.tar.xz \
  "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
tar -xf ffmpeg.tar.xz
cd "ffmpeg-$FFMPEG_VERSION"

# No --enable-gpl and no --enable-nonfree, deliberately. Everything else is
# trimmed because a smaller binary is a smaller download, and Transcrate reads
# and writes a known set of formats.
#
# Two exceptions to the trimming. lavfi, because the integration tests
# synthesise their input with it — keeping it means the binary that ships is the
# binary the suite was run against, rather than one that resembles it. And the
# whole PCM family, because the encoder is chosen from the output's bit depth at
# runtime (`pcm_s24be` and so on), so listing only the depths in use today would
# break the first time an unusual source turned up.
#
# PKG_CONFIG_LIBDIR rather than PKG_CONFIG_PATH: the latter adds to the host's
# search path, and a cross build that picks up the host's libraries fails late
# and confusingly. This way only the prefix just built is visible.
PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" ./configure \
  ${cross[@]:+"${cross[@]}"} \
  --prefix="$prefix" \
  --pkg-config-flags="--static" \
  --extra-cflags="-I$prefix/include" \
  --extra-ldflags="-L$prefix/lib $ldflags" \
  --enable-static \
  --disable-shared \
  --disable-doc \
  --disable-debug \
  --disable-network \
  --disable-autodetect \
  --disable-programs \
  --enable-ffmpeg \
  --enable-ffprobe \
  --enable-libmp3lame \
  --disable-everything \
  --enable-protocol=file,pipe \
  --enable-demuxer=mp3,mov,flac,wav,aiff,aac,ogg,matroska,image2 \
  --enable-muxer=mp3,mp4,ipod,flac,wav,aiff,image2 \
  --enable-decoder=mp3,aac,alac,flac,mjpeg,png \
  --enable-decoder=pcm_u8,pcm_s8,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be \
  --enable-decoder=pcm_s32le,pcm_s32be,pcm_f32le,pcm_f32be,pcm_f64le,pcm_f64be \
  --enable-encoder=libmp3lame,aac,alac,flac,mjpeg,png \
  --enable-encoder=pcm_u8,pcm_s8,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be \
  --enable-encoder=pcm_s32le,pcm_s32be,pcm_f32le,pcm_f32be \
  --enable-parser=mpegaudio,aac,flac,mjpeg,png \
  --enable-filter=aresample,aformat,anull,volume,atrim,format,null,anullsrc,sine \
  --enable-indev=lavfi \
  --enable-bsf=extract_extradata

make -j"$cores"

for tool in ffmpeg ffprobe; do
  cp "$tool$suffix" "$out/"
  "$strip_with" "$out/$tool$suffix"
done

echo "==> built"
ls -la "$out"
