# Imported targets for the FFmpeg built by scripts/build-ffmpeg.sh:
# FFmpeg::avformat, FFmpeg::avcodec, FFmpeg::swresample, FFmpeg::avutil.
# Shared libraries with @rpath install names; see PLAN.md §4.3. With
# ANOMP_BUILD_FUZZERS, the static libraries of build-ffmpeg.sh --fuzz
# instead, instrumented for libFuzzer, ASan and UBSan (PLAN.md H5).

if(APPLE AND NOT IOS AND ANOMP_BUILD_FUZZERS)
    set(_anomp_ffmpeg_platform macos-arm64-fuzz)
elseif(APPLE AND NOT IOS)
    set(_anomp_ffmpeg_platform macos-universal)
else()
    message(FATAL_ERROR "No FFmpeg build for this platform yet (PLAN.md Phases 8-10)")
endif()

set(ANOMP_FFMPEG_DIR "${PROJECT_SOURCE_DIR}/third_party/ffmpeg/${_anomp_ffmpeg_platform}"
    CACHE PATH "FFmpeg install built by scripts/build-ffmpeg.sh")

if(NOT EXISTS "${ANOMP_FFMPEG_DIR}/BUILD_INFO")
    message(FATAL_ERROR
        "FFmpeg not found in ${ANOMP_FFMPEG_DIR}.\n"
        "Run scripts/build-ffmpeg.sh from the repository root first "
        "(with --fuzz for the fuzz preset).")
endif()

# Rebuilding FFmpeg rewrites BUILD_INFO, which re-runs this configure step.
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${ANOMP_FFMPEG_DIR}/BUILD_INFO")

function(_anomp_ffmpeg_import name)
    if(ANOMP_BUILD_FUZZERS)
        add_library(FFmpeg::${name} STATIC IMPORTED GLOBAL)
        set_target_properties(FFmpeg::${name} PROPERTIES
            IMPORTED_LOCATION "${ANOMP_FFMPEG_DIR}/lib/lib${name}.a"
            INTERFACE_INCLUDE_DIRECTORIES "${ANOMP_FFMPEG_DIR}/include")
        return()
    endif()

    # The major-versioned symlink (libavcodec.63.dylib) is the install name.
    file(GLOB _candidates "${ANOMP_FFMPEG_DIR}/lib/lib${name}.[0-9]*.dylib")
    set(_soname "")
    foreach(_candidate IN LISTS _candidates)
        get_filename_component(_file "${_candidate}" NAME)
        if(_file MATCHES "^lib${name}\\.[0-9]+\\.dylib$")
            set(_soname "${_file}")
        endif()
    endforeach()
    if(NOT _soname)
        message(FATAL_ERROR "lib${name} missing from ${ANOMP_FFMPEG_DIR}/lib")
    endif()

    add_library(FFmpeg::${name} SHARED IMPORTED GLOBAL)
    set_target_properties(FFmpeg::${name} PROPERTIES
        IMPORTED_LOCATION "${ANOMP_FFMPEG_DIR}/lib/${_soname}"
        IMPORTED_SONAME "@rpath/${_soname}"
        INTERFACE_INCLUDE_DIRECTORIES "${ANOMP_FFMPEG_DIR}/include")
endfunction()

foreach(_lib IN ITEMS avutil swresample avcodec avformat)
    _anomp_ffmpeg_import(${_lib})
endforeach()

set_property(TARGET FFmpeg::swresample APPEND PROPERTY INTERFACE_LINK_LIBRARIES FFmpeg::avutil)
set_property(TARGET FFmpeg::avcodec APPEND PROPERTY INTERFACE_LINK_LIBRARIES FFmpeg::swresample FFmpeg::avutil)
set_property(TARGET FFmpeg::avformat APPEND PROPERTY INTERFACE_LINK_LIBRARIES FFmpeg::avcodec FFmpeg::avutil)
