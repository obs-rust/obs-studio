# Public-API characterization, unchanged for the later Rust swap.
add_executable(test_source_save_load test_source_save_load.c)
target_include_directories(test_source_save_load PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_save_load PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_save_load)
add_test(NAME test_source_save_load COMMAND test_source_save_load)
set_tests_properties(test_source_save_load PROPERTIES TIMEOUT 30)
