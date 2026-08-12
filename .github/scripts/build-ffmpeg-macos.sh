#!/usr/bin/env bash
#
# Build a static, LGPL ffmpeg and ffprobe for macOS.
#
# Nobody publishes one. Every prebuilt macOS ffmpeg found so far is GPL,
# because they all enable x264 — and Transcrate is MIT or Apache-2.0, so a GPL
# binary inside the same bundle would carry GPL obligations into the thing
# being shipped.
#
# Leaving out --enable-gpl costs nothing here: MP3 comes from libmp3lame,
# which is LGPL, AAC from ffmpeg's own encoder, and FLAC, ALAC and PCM are
# native. Those are every format Transcrate writes.
#
# Usage: build-ffmpeg-macos.sh <output-directory>

set -euo pipefail

out="$(cd "$(dirname "${1:?output directory}")" && pwd)/$(basename "$1")"
mkdir -p "$out"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cd "$work"

# Pinned rather than tracking a moving target: a release should be able to
# rebuild the same binary a year from now.
LAME_VERSION="3.100"
FFMPEG_VERSION="7.1.1"

prefix="$work/prefix"
mkdir -p "$prefix"

echo "==> lame $LAME_VERSION"
curl -fsSL -o lame.tar.gz \
  "https://downloads.sourceforge.net/project/lame/lame/$LAME_VERSION/lame-$LAME_VERSION.tar.gz"
tar -xzf lame.tar.gz
cd "lame-$LAME_VERSION"
# frontend/ builds the `lame` command line tool, which needs nothing we ship
# and fails on macOS over a missing termcap.
./configure --prefix="$prefix" --disable-shared --enable-static --disable-frontend
make -j"$(sysctl -n hw.ncpu)"
make install
cd "$work"

echo "==> ffmpeg $FFMPEG_VERSION"
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
PKG_CONFIG_PATH="$prefix/lib/pkgconfig" ./configure \
  --prefix="$prefix" \
  --pkg-config-flags="--static" \
  --extra-cflags="-I$prefix/include" \
  --extra-ldflags="-L$prefix/lib" \
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

make -j"$(sysctl -n hw.ncpu)"

cp ffmpeg ffprobe "$out/"
strip "$out/ffmpeg" "$out/ffprobe"

echo "==> built"
"$out/ffmpeg" -version | head -2
ls -la "$out"
