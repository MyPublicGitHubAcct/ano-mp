# anomp_core: the JUCE-based audio engine, exposed to the Tauri (Rust) layer
# through the plain C API in include/anomp/anomp.h.
add_library(anomp_core STATIC
    src/AudioEngine.cpp
    src/FFmpegAudioFormat.cpp
    src/FormatRegistry.cpp
    src/PlayerEngine.cpp
    src/anomp_c_api.cpp)

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
        JUCE_STANDALONE_APPLICATION=0)

target_link_libraries(anomp_core
    PUBLIC
        juce::juce_audio_formats
        juce::juce_audio_devices
    PRIVATE
        FFmpeg::avformat
        juce::juce_recommended_config_flags
        juce::juce_recommended_warning_flags)
