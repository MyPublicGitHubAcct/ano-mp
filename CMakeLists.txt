cmake_minimum_required(VERSION 3.25)

# Must precede project() to take effect; minimum OS targets are in PLAN.md §4.5.
set(CMAKE_OSX_DEPLOYMENT_TARGET "14.0" CACHE STRING "Minimum macOS version")

project(ano_mp VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

option(ANOMP_BUILD_TESTS "Build the Catch2 test suite" ON)

include(FetchContent)

# JUCE and Catch2 are pinned by commit, not tag, since a tag can move
# (PLAN.md H7): GitHub's archive of the commit the release tag named, with
# its SHA-256, like TagLib's and Signalsmith's tarballs. (A shallow git
# clone can't fetch a commit by hash, and a full clone of JUCE is large.)
# JUCE 9.0.2
FetchContent_Declare(JUCE
    URL      https://github.com/juce-framework/JUCE/archive/72782788ce18c2d4d760b28e0921d6ffc6431102.tar.gz
    URL_HASH SHA256=28ec8ae0626a4d1c6ca8c32ad13f95868b147779d4f3df4a7868f333a38e0551
    DOWNLOAD_EXTRACT_TIMESTAMP OFF)
FetchContent_MakeAvailable(JUCE)

include(cmake/FFmpeg.cmake)
include(cmake/TagLib.cmake)
include(cmake/Signalsmith.cmake)

add_subdirectory(core)

if(ANOMP_BUILD_TESTS)
    # Catch2 v3.16.0
    FetchContent_Declare(Catch2
        URL      https://github.com/catchorg/Catch2/archive/317ac1ed4c0bb6e6b91eafc817e05c488feffcb3.tar.gz
        URL_HASH SHA256=4f5be6862411b13785213aefe0581380412792bce78bc973ead4983f62b6361d
        DOWNLOAD_EXTRACT_TIMESTAMP OFF)
    FetchContent_MakeAvailable(Catch2)

    enable_testing()
    add_subdirectory(core/tests)
endif()
