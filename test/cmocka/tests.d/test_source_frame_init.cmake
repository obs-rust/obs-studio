# Public frame initialization, characterized before its Rust swap (#5619).
# Uses no core or host services, and runs against both libobs backends.
add_executable(test_source_frame_init test_source_frame_init.c)
target_include_directories(test_source_frame_init PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_frame_init PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_frame_init)
add_test(NAME test_source_frame_init COMMAND test_source_frame_init)
set_tests_properties(test_source_frame_init PROPERTIES TIMEOUT 30)
