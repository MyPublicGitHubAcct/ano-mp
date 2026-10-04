add_executable(anomp_core_tests
    AnalysisTests.cpp
    BenchTests.cpp
    FormatRegistryTests.cpp
    CApiTests.cpp
    FFmpegAudioFormatTests.cpp
    FFmpegBuildTests.cpp
    FileAnalysisTests.cpp
    FolderAccessTests.cpp
    MediaControlsTests.cpp
    PlayerEngineTests.cpp
    TagReaderTests.cpp)

if(APPLE AND NOT CMAKE_SYSTEM_NAME STREQUAL "iOS")
    target_sources(anomp_core_tests PRIVATE VolumeWatcherTests.mm)
    set_source_files_properties(VolumeWatcherTests.mm PROPERTIES COMPILE_OPTIONS -fobjc-arc)
endif()

target_compile_definitions(anomp_core_tests
    PRIVATE ANOMP_TEST_FIXTURES_DIR="${CMAKE_CURRENT_SOURCE_DIR}/fixtures"
            ANOMP_PROJECT_VERSION="${ano_mp_VERSION}")

target_include_directories(anomp_core_tests PRIVATE ../src)

target_link_libraries(anomp_core_tests
    PRIVATE
        anomp_core
        anomp_effects
        FFmpeg::avformat
        TagLib::tag
        Catch2::Catch2WithMain)

include(Catch)
catch_discover_tests(anomp_core_tests)
