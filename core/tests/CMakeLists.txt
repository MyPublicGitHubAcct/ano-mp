add_executable(anomp_core_tests
    AnalysisTests.cpp
    FormatRegistryTests.cpp
    CApiTests.cpp
    FFmpegAudioFormatTests.cpp
    FFmpegBuildTests.cpp
    FileAnalysisTests.cpp
    FolderAccessTests.cpp
    MediaControlsTests.cpp
    PlayerEngineTests.cpp
    TagReaderTests.cpp)

target_compile_definitions(anomp_core_tests
    PRIVATE ANOMP_TEST_FIXTURES_DIR="${CMAKE_CURRENT_SOURCE_DIR}/fixtures")

target_include_directories(anomp_core_tests PRIVATE ../src)

target_link_libraries(anomp_core_tests
    PRIVATE
        anomp_core
        FFmpeg::avformat
        TagLib::tag
        Catch2::Catch2WithMain)

include(Catch)
catch_discover_tests(anomp_core_tests)
