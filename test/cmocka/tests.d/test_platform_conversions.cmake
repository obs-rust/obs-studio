# Characterization of libobs/util/platform-conversions.c before its port (#262 slice 1).
add_executable(test_platform_conversions test_platform_conversions.c)
target_include_directories(test_platform_conversions PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_platform_conversions PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_platform_conversions)
add_test(test_platform_conversions ${CMAKE_CURRENT_BINARY_DIR}/test_platform_conversions)
