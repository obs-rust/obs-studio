# Characterization of libobs/util/platform-file.c before its port (#262 slice 3).
add_executable(test_platform_file test_platform_file.c)
target_include_directories(test_platform_file PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_platform_file PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_platform_file)
add_test(test_platform_file ${CMAKE_CURRENT_BINARY_DIR}/test_platform_file)
