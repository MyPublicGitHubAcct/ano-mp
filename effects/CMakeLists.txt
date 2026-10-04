# anomp_effects: the real-time effects (PLAN.md X2), a library of its own in
# plain C++20 with no dependencies, which the core links. Its one public
# header is include/anomp/effects/EffectChain.h.
add_library(anomp_effects STATIC
    src/Echo.cpp
    src/EffectChain.cpp
    src/Fft.cpp
    src/Modulation.cpp
    src/Reverb.cpp
    src/SpectralFreeze.cpp)

target_include_directories(anomp_effects
    PUBLIC  include
    PRIVATE src)

target_compile_features(anomp_effects PUBLIC cxx_std_20)

# The same warnings and optimisation as the core's: JUCE's flag targets are
# interface libraries of compiler options only, which compile no JUCE code
# into this library. Built without JUCE, it takes the compiler's defaults.
if(TARGET juce::juce_recommended_warning_flags)
    target_link_libraries(anomp_effects PRIVATE
        juce::juce_recommended_config_flags
        juce::juce_recommended_warning_flags)
endif()
