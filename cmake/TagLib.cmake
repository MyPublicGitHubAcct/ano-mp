# TagLib reads tags and embedded art (PLAN.md §4.4). It is used under the MPL,
# built as a static library from the pinned release tarball, which bundles
# utfcpp. Exposes the target TagLib::tag.

anomp_fetch_declare(taglib
    URL      https://github.com/taglib/taglib/releases/download/v2.3.2/taglib-2.3.2.tar.gz
    URL_HASH SHA256=3ca2d8afaa7f1cf7f6ed10e511ebc368bfacd6dcaa3dbfa690b89e502e8963dc)

block(SCOPE_FOR VARIABLES PROPAGATE taglib_SOURCE_DIR taglib_BINARY_DIR)
    set(BUILD_SHARED_LIBS OFF)
    set(BUILD_TESTING OFF)
    set(BUILD_EXAMPLES OFF)
    set(BUILD_BINDINGS OFF)
    # zlib only inflates compressed ID3v2 frames, which are rare; without it
    # TagLib skips them. Leaving it out keeps the link line unchanged.
    set(WITH_ZLIB OFF)
    # Only the formats FFmpeg plays (scripts/build-ffmpeg.sh): TagLib
    # otherwise detects the others by content, and every parser is attack
    # surface (PLAN.md H5; the fuzzer found UB in the Shorten reader). APE
    # stays: MP3 files can carry APE tags.
    set(WITH_SHORTEN OFF)
    set(WITH_TRUEAUDIO OFF)
    set(WITH_DSF OFF)
    set(WITH_MOD OFF)
    set(WITH_MATROSKA OFF)
    # Always the bundled utfcpp, never one installed on the build machine.
    set(CMAKE_DISABLE_FIND_PACKAGE_utf8cpp ON)
    set(utf8cpp_INCLUDE_DIR "${FETCHCONTENT_BASE_DIR}/taglib-src/3rdparty/utfcpp/source"
        CACHE PATH "utfcpp bundled with TagLib" FORCE)
    FetchContent_MakeAvailable(taglib)
endblock()

# TagLib's own target only declares include directories for installed builds.
# Installed headers are flat, and include each other by bare name, so the
# format folders the core reads format-specific frames from (ID3v2 SYLT and
# POPM in MPEG, AIFF, WAV and FLAC files, MP4's rate) and names the kinds of
# tag of (TagReader's readFileInfo) are listed too.
target_include_directories(tag SYSTEM INTERFACE
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/toolkit>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/mpeg>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/mpeg/id3v2>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/mpeg/id3v2/frames>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/riff>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/riff/aiff>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/riff/wav>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/flac>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/mp4>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/ogg>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/ogg/xiph>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/ape>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/wavpack>
    $<BUILD_INTERFACE:${taglib_SOURCE_DIR}/taglib/asf>
    $<BUILD_INTERFACE:${taglib_BINARY_DIR}>)

add_library(TagLib::tag ALIAS tag)
