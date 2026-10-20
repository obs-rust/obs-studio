# Characterization of libobs/util/platform-time.c before its port (#262 slice 6).
add_executable(test_platform_time test_platform_time.c)
target_include_directories(test_platform_time PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_platform_time PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_platform_time)
add_test(test_platform_time ${CMAKE_CURRENT_BINARY_DIR}/test_platform_time)
