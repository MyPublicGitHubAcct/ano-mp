add_executable(anomp_core_tests
    FormatRegistryTests.cpp
    CApiTests.cpp)

target_include_directories(anomp_core_tests PRIVATE ../src)

target_link_libraries(anomp_core_tests
    PRIVATE
        anomp_core
        Catch2::Catch2WithMain)

include(Catch)
catch_discover_tests(anomp_core_tests)
