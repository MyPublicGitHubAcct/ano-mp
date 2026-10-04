#!/usr/bin/env bash
# Builds the pinned FFmpeg that anomp_core decodes with (PLAN.md §4.3):
# LGPL, audio-only, shared libraries, no programs, network or video. Its
# encoders and muxers are only those recording writes (PLAN.md X6); MP3's is
# LAME, built here as a static library linked into libavcodec (LGPL too).
#
# Output: third_party/ffmpeg/<platform>/{include,lib,BUILD_INFO}
#   macos-universal   arm64 + x86_64 merged with lipo
#   macos-arm64-fuzz  with --fuzz: static, arm64 only, compiled by Homebrew's
#                     llvm@22 with libFuzzer coverage, ASan and UBSan, for the
#                     CMake `fuzz` preset (PLAN.md H5); the same pin and formats
# Phases 8-10 add iOS, Linux and Windows.
#
# Usage: scripts/build-ffmpeg.sh [--force] [--fuzz] [--info]
# Skips the build when BUILD_INFO already matches this script's version and
# flags; --force rebuilds anyway. CI caches the output keyed on BUILD_INFO.
# --info prints the BUILD_INFO a build would write and builds nothing
# (scripts/doctor.py compares it with the one built).

set -euo pipefail

FFMPEG_VERSION="9.0.2"
# Checked against the tarball's GPG signature (FFmpeg release signing key
# FCF9 86EA 15E6 E293 A564 4F10 B432 2F04 D676 58D8) when the pin was set.
FFMPEG_SHA256="8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e"
MACOS_MIN="14.0" # PLAN.md §4.5

# LAME, MP3's encoder for recordings (PLAN.md X6). SourceForge publishes
# no signature, so the hash was checked against Homebrew's formula when the
# pin was set (2026-10-04).
LAME_VERSION="4.0"
LAME_SHA256="3df5124d5ad3a98312ffd7ba6a9b36230e4f8a3e66d3ce0f425e336c32d216eb"

# Formats: MP3, FLAC, WAV, AIFF, Ogg Vorbis, Opus, AAC/M4A, ALAC, WMA.
DEMUXERS="mp3,flac,wav,aiff,ogg,mov,asf,aac"
DECODERS="mp3float,flac,vorbis,opus,aac,alac,wmav1,wmav2,wmapro,wmalossless"
DECODERS+=",pcm_s8,pcm_u8,pcm_s16le,pcm_s16be,pcm_s24le,pcm_s24be,pcm_s32le,pcm_s32be"
DECODERS+=",pcm_f32le,pcm_f32be,pcm_f64le,pcm_f64be,pcm_alaw,pcm_mulaw"
PARSERS="mpegaudio,flac,vorbis,opus,aac"
# Recording (PLAN.md X6): WAV, AIFF, FLAC, ALAC and AAC in M4A, and MP3
# (libmp3lame, in the macOS build's LAME_FLAGS; the fuzz build has none).
ENCODERS="pcm_s16le,pcm_s24le,pcm_f32le,pcm_s16be,pcm_s24be,flac,alac,aac"
MUXERS="wav,aiff,flac,ipod,mp3"

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
    --enable-encoder="$ENCODERS"
    --enable-muxer="$MUXERS"
)
LAME_FLAGS=(
    --enable-libmp3lame
    --enable-encoder=libmp3lame
)

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="$REPO_ROOT/build/ffmpeg"
OUT_ROOT="$REPO_ROOT/third_party/ffmpeg"
JOBS="$(sysctl -n hw.ncpu 2>/dev/null || nproc)"

# The fuzz build's compiler (Apple's clang has no libFuzzer runtime) and
# flags. The fuzz preset in CMakePresets.json names the same compiler.
FUZZ_LLVM="/opt/homebrew/opt/llvm@22"
FUZZ_FLAGS="-fsanitize=fuzzer-no-link,address,undefined -fno-sanitize-recover=undefined -fno-omit-frame-pointer -g"

FORCE=0
FUZZ=0
INFO=0
for arg in "$@"; do
    case "$arg" in
        --force) FORCE=1 ;;
        --fuzz) FUZZ=1 ;;
        --info) INFO=1 ;;
        *) printf 'usage: %s [--force] [--fuzz] [--info]\n' "$0" >&2; exit 2 ;;
    esac
done

log() { printf '\n==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

build_info() {
    if [[ "$1" == *-fuzz ]]; then
        printf 'ffmpeg %s\nsha256 %s\nplatform %s\nmin_os %s\nconfigure %s\n' \
            "$FFMPEG_VERSION" "$FFMPEG_SHA256" "$1" "$MACOS_MIN" "${CONFIGURE_FLAGS[*]}"
        printf 'compiler %s\nfuzz %s\n' "$("$FUZZ_LLVM/bin/clang" --version | head -1)" "$FUZZ_FLAGS"
    else
        printf 'ffmpeg %s\nsha256 %s\nplatform %s\nmin_os %s\nconfigure %s\nlame %s\nlame_sha256 %s\n' \
            "$FFMPEG_VERSION" "$FFMPEG_SHA256" "$1" "$MACOS_MIN" "${CONFIGURE_FLAGS[*]} ${LAME_FLAGS[*]}" \
            "$LAME_VERSION" "$LAME_SHA256"
    fi
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

fetch_lame() {
    local tarball="$WORK_DIR/lame-$LAME_VERSION.tar.gz"
    mkdir -p "$WORK_DIR"
    if [[ ! -f "$tarball" ]]; then
        log "Downloading LAME $LAME_VERSION"
        curl -fSL --retry 3 -o "$tarball.part" \
            "https://downloads.sourceforge.net/project/lame/lame/$LAME_VERSION/lame-$LAME_VERSION.tar.gz"
        mv "$tarball.part" "$tarball"
    fi
    echo "$LAME_SHA256  $tarball" | shasum -a 256 -c - >/dev/null \
        || die "checksum mismatch for $tarball (delete it to re-download)"

    LAME_SRC_DIR="$WORK_DIR/lame-$LAME_VERSION"
    rm -rf "$LAME_SRC_DIR"
    tar -xzf "$tarball" -C "$WORK_DIR"
}

# build_lame_arch <arch> <prefix>: libmp3lame.a only (no frontend or decoder).
build_lame_arch() {
    local arch="$1" prefix="$2"
    local build_dir="$WORK_DIR/build-lame-$arch"
    local host="aarch64-apple-darwin"
    [[ "$arch" == x86_64 ]] && host="x86_64-apple-darwin"

    log "Building LAME for macOS $arch"
    rm -rf "$build_dir" "$prefix"
    mkdir -p "$build_dir"
    (
        cd "$build_dir"
        CC=clang CFLAGS="-arch $arch -mmacosx-version-min=$MACOS_MIN -O2 -fPIC" \
            "$LAME_SRC_DIR/configure" \
            --prefix="$prefix" \
            --host="$host" \
            --disable-shared \
            --enable-static \
            --disable-frontend \
            --disable-decoder \
            --disable-gtktest \
            --disable-dependency-tracking > configure.log 2>&1 \
            || { tail -30 configure.log; die "LAME configure failed for $arch"; }
        make -j"$JOBS" > build.log 2>&1 || { tail -30 build.log; die "LAME build failed for $arch"; }
        make install > install.log 2>&1 || { tail -30 install.log; die "LAME install failed for $arch"; }
    )
}

# build_macos_arch <arch> <prefix>
build_macos_arch() {
    local arch="$1" prefix="$2"
    local build_dir="$WORK_DIR/build-macos-$arch"
    local lame="$WORK_DIR/lame-macos-$arch"
    local flags="-arch $arch -mmacosx-version-min=$MACOS_MIN"

    build_lame_arch "$arch" "$lame"

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
            --extra-cflags="$flags -I$lame/include" \
            --extra-ldflags="$flags -L$lame/lib" \
            "${CONFIGURE_FLAGS[@]}" \
            "${LAME_FLAGS[@]}" > configure.log \
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
    fetch_lame
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

# Static libraries, so libFuzzer's coverage hooks resolve when a fuzz target
# links them; the host architecture only, since fuzzers run where they build.
build_macos_fuzz() {
    local out="$OUT_ROOT/macos-arm64-fuzz"
    local build_dir="$WORK_DIR/build-macos-arm64-fuzz"
    local info
    [[ -x "$FUZZ_LLVM/bin/clang" ]] || die "$FUZZ_LLVM/bin/clang not found: brew install llvm@22"
    [[ "$(uname -m)" == arm64 ]] || die "the fuzz build is arm64 only"
    info="$(build_info macos-arm64-fuzz)"

    if [[ $FORCE -eq 0 && -f "$out/BUILD_INFO" && "$(cat "$out/BUILD_INFO")" == "$info" ]]; then
        log "FFmpeg $FFMPEG_VERSION fuzz build is up to date ($out)"
        return
    fi

    fetch_source
    local flags="-arch arm64 -mmacosx-version-min=$MACOS_MIN -isysroot $(xcrun --show-sdk-path)"

    log "Configuring FFmpeg's fuzz build"
    rm -rf "$build_dir" "$out"
    mkdir -p "$build_dir"
    (
        cd "$build_dir"
        "$SRC_DIR/configure" \
            --prefix="$out" \
            --cc="$FUZZ_LLVM/bin/clang" \
            --extra-cflags="$flags $FUZZ_FLAGS" \
            --extra-ldflags="$flags $FUZZ_FLAGS" \
            "${CONFIGURE_FLAGS[@]}" \
            --disable-shared \
            --enable-static \
            --disable-stripping > configure.log \
            || { tail -30 configure.log; die "configure failed (see $build_dir/configure.log)"; }

        grep -q '^License: LGPL' configure.log || die "the fuzz build would not be LGPL; check the configure flags"

        log "Building FFmpeg's fuzz build"
        make -j"$JOBS" > build.log 2>&1 || { tail -30 build.log; die "build failed"; }
        make install > install.log 2>&1 || { tail -30 install.log; die "install failed"; }
    )

    echo "$info" > "$out/BUILD_INFO"
    log "Done: $out"
}

case "$(uname -s)" in
    Darwin)
        if [[ $INFO -eq 1 ]]; then
            if [[ $FUZZ -eq 1 ]]; then build_info macos-arm64-fuzz; else build_info macos-universal; fi
        elif [[ $FUZZ -eq 1 ]]; then build_macos_fuzz; else build_macos; fi ;;
    *) die "unsupported host $(uname -s); Linux and Windows arrive in Phases 9-10" ;;
esac
