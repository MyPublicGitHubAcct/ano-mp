# anomp_core: the JUCE-based audio engine, exposed to the Tauri (Rust) layer
# through the plain C API in include/anomp/anomp.h.
add_library(anomp_core STATIC
    src/AnalysisThread.cpp
    src/AudioEngine.cpp
    src/Crossfeed.cpp
    src/DockMenu.cpp
    src/Equaliser.cpp
    src/FFmpegAudioFormat.cpp
    src/FileAnalyser.cpp
    src/FormatRegistry.cpp
    src/Log.cpp
    src/MediaControls.cpp
    src/PlayerEngine.cpp
    src/SpectrumAnalyser.cpp
    src/TagReader.cpp
    src/anomp_c_api.cpp)

# Platform code, one implementation per OS (PLAN.md §5).
if(APPLE)
    target_sources(anomp_core PRIVATE src/DockMenu_apple.mm src/FolderAccess_apple.mm src/MediaControls_apple.mm
        src/OutputRoute_apple.mm src/VolumeWatcher_apple.mm)
    set_source_files_properties(src/DockMenu_apple.mm src/FolderAccess_apple.mm src/MediaControls_apple.mm
        src/OutputRoute_apple.mm src/VolumeWatcher_apple.mm PROPERTIES COMPILE_OPTIONS -fobjc-arc)
    # Hosts that link the core by hand (app/src-tauri/build.rs) list these too.
    target_link_libraries(anomp_core PRIVATE "-framework MediaPlayer")
    if(CMAKE_SYSTEM_NAME STREQUAL "iOS")
        target_link_libraries(anomp_core PRIVATE "-framework UIKit")
    else()
        target_link_libraries(anomp_core PRIVATE "-framework AppKit")
    endif()
else()
    target_sources(anomp_core PRIVATE src/DockMenu_none.cpp src/FolderAccess_unsandboxed.cpp src/MediaControls_none.cpp
        src/OutputRoute_none.cpp src/VolumeWatcher_none.cpp)
endif()

target_include_directories(anomp_core
    PUBLIC  include
    PRIVATE src)

target_compile_definitions(anomp_core
    PUBLIC
        JUCE_WEB_BROWSER=0
        JUCE_USE_CURL=0
        # FFmpeg decodes everything; JUCE's own decoders are not built.
        JUCE_USE_MP3AUDIOFORMAT=0
        JUCE_USE_FLAC=0
        JUCE_USE_OGGVORBIS=0
        JUCE_STANDALONE_APPLICATION=0
        # Failed assertions go to the log in every build (Log.h, PLAN.md H9).
        JUCE_LOG_ASSERTIONS=1)

target_link_libraries(anomp_core
    PUBLIC
        juce::juce_audio_formats
        juce::juce_audio_devices
        # FFT for the visualizer's analysis (SpectrumAnalyser).
        juce::juce_dsp
    PRIVATE
        FFmpeg::avformat
        TagLib::tag
        Signalsmith::stretch
        juce::juce_recommended_config_flags
        juce::juce_recommended_warning_flags)
