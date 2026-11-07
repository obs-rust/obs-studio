# Actual-library characterization of obs-module's failure table ownership.
add_executable(test_module_failure_info test_module_failure_info.c)
target_include_directories(test_module_failure_info PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_module_failure_info PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_module_failure_info)
add_test(test_module_failure_info ${CMAKE_CURRENT_BINARY_DIR}/test_module_failure_info)
set_tests_properties(test_module_failure_info PROPERTIES TIMEOUT 30)
