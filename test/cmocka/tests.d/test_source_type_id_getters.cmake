# Public identity getters against the actual hybrid and C-reference libraries.
add_executable(test_source_type_id_getters test_source_type_id_getters.c)
target_include_directories(test_source_type_id_getters PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_type_id_getters PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_type_id_getters)
add_test(NAME test_source_type_id_getters COMMAND test_source_type_id_getters)
set_tests_properties(test_source_type_id_getters PROPERTIES TIMEOUT 30)
