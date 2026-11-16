# Source type lookup through the public calls, against hybrid and C-reference libobs.
add_executable(test_source_info_lookup test_source_info_lookup.c)
target_include_directories(test_source_info_lookup PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_info_lookup PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_info_lookup)
add_test(NAME test_source_info_lookup COMMAND test_source_info_lookup)
set_tests_properties(test_source_info_lookup PROPERTIES TIMEOUT 30)
