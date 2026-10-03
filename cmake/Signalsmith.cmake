# Signalsmith Stretch changes tempo without changing pitch, and pitch without
# changing tempo, for practice mode (PLAN.md O12). It and the FFT library it
# needs are header-only and MIT-licensed, so they link statically on every
# platform, iOS included. Fetched from their pinned release tarballs; their
# own CMake files (which fetch from git) are not used. Exposes the interface
# target Signalsmith::stretch.

anomp_fetch_declare(signalsmith_stretch
    URL      https://github.com/Signalsmith-Audio/signalsmith-stretch/archive/refs/tags/1.4.0.tar.gz
    URL_HASH SHA256=077235709ecf2a358545e3ca7bdf32859f70f5c9afb4493e078bd753c9aff284
    SOURCE_SUBDIR  no-cmake)
anomp_fetch_declare(signalsmith_linear
    URL      https://github.com/Signalsmith-Audio/linear/archive/refs/tags/0.6.4.tar.gz
    URL_HASH SHA256=2cb10d84b96c626255ab46c4aa4c80f64ae532be487063cff6a08903c1445a56
    SOURCE_SUBDIR  no-cmake)
FetchContent_MakeAvailable(signalsmith_stretch signalsmith_linear)

add_library(signalsmith_stretch INTERFACE)
# SYSTEM: their warnings aren't ours to fix, and the core builds warning-clean.
target_include_directories(signalsmith_stretch SYSTEM INTERFACE
    ${signalsmith_stretch_SOURCE_DIR}/include
    ${signalsmith_linear_SOURCE_DIR}/include)
if(APPLE)
    # Accelerate's FFT; the app already links the framework for JUCE.
    target_compile_definitions(signalsmith_stretch INTERFACE SIGNALSMITH_USE_ACCELERATE ACCELERATE_NEW_LAPACK)
    target_link_libraries(signalsmith_stretch INTERFACE "-framework Accelerate")
endif()
add_library(Signalsmith::stretch ALIAS signalsmith_stretch)
