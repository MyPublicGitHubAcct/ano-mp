add_executable(anomp_effects_tests
    ChainTests.cpp
    EffectTests.cpp
    FftTests.cpp
    FreezeTests.cpp)

# The tests reach the effects themselves, behind the public header.
target_include_directories(anomp_effects_tests PRIVATE ../src)

target_link_libraries(anomp_effects_tests
    PRIVATE
        anomp_effects
        Catch2::Catch2WithMain)

include(Catch)
catch_discover_tests(anomp_effects_tests)
