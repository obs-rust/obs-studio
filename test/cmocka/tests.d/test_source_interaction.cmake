# Freeze source interaction dispatch before its Rust replacement.
add_executable(test_source_interaction test_source_interaction.c)
target_include_directories(test_source_interaction PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_source_interaction PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_source_interaction)
add_test(NAME test_source_interaction COMMAND test_source_interaction)
set_tests_properties(test_source_interaction PROPERTIES TIMEOUT 30)
