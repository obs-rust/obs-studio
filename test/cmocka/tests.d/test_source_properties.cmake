# Freeze the C source property constructors before their Rust replacement.
add_executable(test_source_properties test_source_properties.c)
target_include_directories(test_source_properties PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_properties PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_properties)
add_test(NAME test_source_properties COMMAND test_source_properties)
set_tests_properties(test_source_properties PROPERTIES TIMEOUT 30)
