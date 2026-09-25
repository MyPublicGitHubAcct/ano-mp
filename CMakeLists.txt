cmake_minimum_required(VERSION 3.25)

project(ano_mp VERSION 0.1.0 LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_OSX_DEPLOYMENT_TARGET "12.0" CACHE STRING "Minimum macOS version")

option(ANOMP_BUILD_TESTS "Build the Catch2 test suite" ON)

include(FetchContent)

FetchContent_Declare(JUCE
    GIT_REPOSITORY https://github.com/juce-framework/JUCE.git
    GIT_TAG        9.0.2
    GIT_SHALLOW    ON)
FetchContent_MakeAvailable(JUCE)

add_subdirectory(core)

if(ANOMP_BUILD_TESTS)
    FetchContent_Declare(Catch2
        GIT_REPOSITORY https://github.com/catchorg/Catch2.git
        GIT_TAG        v3.16.0
        GIT_SHALLOW    ON)
    FetchContent_MakeAvailable(Catch2)

    enable_testing()
    add_subdirectory(core/tests)
endif()
