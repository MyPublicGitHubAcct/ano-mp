# libFuzzer targets for the core's parsers (PLAN.md H5), built by the `fuzz`
# preset only, which compiles everything with Homebrew's llvm@22 (Apple's
# clang has no libFuzzer runtime) and links FFmpeg's instrumented static
# build (scripts/build-ffmpeg.sh --fuzz). Run them with scripts/run-fuzzers.py.
add_executable(anomp_decoderfuzzer DecoderFuzzer.cpp)
add_executable(anomp_tagreaderfuzzer TagReaderFuzzer.cpp)

foreach(_target IN ITEMS anomp_decoderfuzzer anomp_tagreaderfuzzer)
    target_include_directories(${_target} PRIVATE ../src)
    target_link_libraries(${_target} PRIVATE anomp_core FFmpeg::avformat TagLib::tag)
    target_link_options(${_target} PRIVATE -fsanitize=fuzzer)
endforeach()
