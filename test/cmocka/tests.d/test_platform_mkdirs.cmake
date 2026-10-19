# Characterization of libobs/util/platform-mkdirs.c before its port (#262 slice 4).
add_executable(test_platform_mkdirs test_platform_mkdirs.c)
target_include_directories(test_platform_mkdirs PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_platform_mkdirs PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_platform_mkdirs)
add_test(test_platform_mkdirs ${CMAKE_CURRENT_BINARY_DIR}/test_platform_mkdirs)
