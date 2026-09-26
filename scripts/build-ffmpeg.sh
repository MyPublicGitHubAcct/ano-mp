#!/usr/bin/env bash
# Builds the pinned FFmpeg that anomp_core decodes with (PLAN.md §4.3):
# LGPL, audio-only, shared libraries, no programs, network, video or encoders.
#
# Output: third_party/ffmpeg/<platform>/{include,lib,BUILD_INFO}
#   macos-universal   arm64 + x86_64 merged with lipo
# Phases 8-10 add iOS, Linux and Windows.
#
# Usage: scripts/build-ffmpeg.sh [--force]
# Skips the build when BUILD_INFO already matches this script's version and
# flags; --force rebuilds anyway. CI caches the output keyed on BUILD_INFO.

set -euo pipefail

FFMPEG_VERSION="9.0.2"
# Checked against the tarball's GPG signature (FFmpeg release signing key
# FCF9 86EA 15E6 E293 A564 4F10 B432 2F04 D676 58D8) when the pin was set.
FFMPEG_SHA256="8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e"
MACOS_MIN="14.0" # PLAN.md §4.5

# Formats: MP3, FLAC, WAV, AIFF, Ogg Vorbis, Opus, AAC/M4A, ALAC, WMA.
DEMUXERS="mp3,flac,wav,aiff,ogg,mov,asf,aac"
DECODERS="mp3float,flac,vorbis,opus,aac,alac,wmav1,wmav2,wmapro,wmalossless"
DECODERS+=",pcm_s8,pcm_u8,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be"
DECODERS+=",pcm_f32le,pcm_f32be,pcm_f64le,pcm_f64be,pcm_alaw,pcm_mulaw"
PARSERS="mpegaudio,flac,vorbis,opus,aac"

CONFIGURE_FLAGS=(
    --disable-everything
    --disable-autodetect
    --disable-programs
    --disable-doc
    --disable-debug
    --disable-network
    --disable-avdevice
    --disable-avfilter
    --disable-swscale
    --enable-avformat
    --enable-avcodec
    --enable-swresample
    --enable-shared
    --disable-static
    --enable-pic
    --enable-protocol=file
    --enable-demuxer="$DEMUXERS"
    --enable-decoder="$DECODERS"
    --enable-parser="$PARSERS"
)

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="$REPO_ROOT/build/ffmpeg"
OUT_ROOT="$REPO_ROOT/third_party/ffmpeg"
JOBS="$(sysctl -n hw.ncpu 2>/dev/null || nproc)"

FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1

log() { printf '\n==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

build_info() {
    printf 'ffmpeg %s\nsha256 %s\nplatform %s\nmin_os %s\nconfigure %s\n' \
        "$FFMPEG_VERSION" "$FFMPEG_SHA256" "$1" "$MACOS_MIN" "${CONFIGURE_FLAGS[*]}"
}

fetch_source() {
    local tarball="$WORK_DIR/ffmpeg-$FFMPEG_VERSION.tar.xz"
    mkdir -p "$WORK_DIR"
    if [[ ! -f "$tarball" ]]; then
        log "Downloading FFmpeg $FFMPEG_VERSION"
        curl -fSL --retry 3 -o "$tarball.part" \
            "https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
        mv "$tarball.part" "$tarball"
    fi
    echo "$FFMPEG_SHA256  $tarball" | shasum -a 256 -c - >/dev/null \
        || die "checksum mismatch for $tarball (delete it to re-download)"

    SRC_DIR="$WORK_DIR/ffmpeg-$FFMPEG_VERSION"
    rm -rf "$SRC_DIR"
    tar -xJf "$tarball" -C "$WORK_DIR"
}

# build_macos_arch <arch> <prefix>
build_macos_arch() {
    local arch="$1" prefix="$2"
    local build_dir="$WORK_DIR/build-macos-$arch"
    local flags="-arch $arch -mmacosx-version-min=$MACOS_MIN"

    log "Configuring FFmpeg for macOS $arch"
    rm -rf "$build_dir" "$prefix"
    mkdir -p "$build_dir"
    (
        cd "$build_dir"
        "$SRC_DIR/configure" \
            --prefix="$prefix" \
            --install-name-dir='@rpath' \
            --enable-cross-compile \
            --target-os=darwin \
            --arch="$arch" \
            --cc="clang" \
            --extra-cflags="$flags" \
            --extra-ldflags="$flags" \
            "${CONFIGURE_FLAGS[@]}" > configure.log \
            || { tail -30 configure.log; die "configure failed for $arch (see $build_dir/configure.log)"; }

        grep -q '^License: LGPL' configure.log \
            || die "FFmpeg $arch would not be LGPL; check the configure flags"

        log "Building FFmpeg for macOS $arch"
        make -j"$JOBS" > build.log 2>&1 || { tail -30 build.log; die "build failed for $arch"; }
        make install > install.log 2>&1 || { tail -30 install.log; die "install failed for $arch"; }
    )
}

build_macos() {
    local out="$OUT_ROOT/macos-universal"
    local info
    info="$(build_info macos-universal)"

    if [[ $FORCE -eq 0 && -f "$out/BUILD_INFO" && "$(cat "$out/BUILD_INFO")" == "$info" ]]; then
        log "FFmpeg $FFMPEG_VERSION for macos-universal is up to date ($out)"
        return
    fi

    fetch_source
    local arm="$WORK_DIR/install-macos-arm64" x86="$WORK_DIR/install-macos-x86_64"
    build_macos_arch arm64 "$arm"
    build_macos_arch x86_64 "$x86"

    log "Merging arm64 and x86_64 into $out"
    # Headers must be identical apart from the install prefix, or one
    # architecture would compile against the other's configuration.
    diff -r "$arm/include" "$x86/include" > /dev/null \
        || die "arm64 and x86_64 headers differ; cannot merge"

    rm -rf "$out"
    mkdir -p "$out/lib"
    cp -R "$arm/include" "$out/include"

    local lib
    for lib in "$arm"/lib/*.dylib; do
        local name
        name="$(basename "$lib")"
        if [[ -L "$lib" ]]; then
            cp -P "$lib" "$out/lib/$name"
        else
            lipo -create "$lib" "$x86/lib/$name" -output "$out/lib/$name"
        fi
    done

    echo "$info" > "$out/BUILD_INFO"
    log "Done: $out"
    lipo -info "$out"/lib/libavcodec.*.*.*.dylib
}

case "$(uname -s)" in
    Darwin) build_macos ;;
    *) die "unsupported host $(uname -s); Linux and Windows arrive in Phases 9-10" ;;
esac
