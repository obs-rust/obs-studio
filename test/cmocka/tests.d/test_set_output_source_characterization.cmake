# The exported obs_set_output_source is characterized in the real library before
# its swap. obs_startup() alone is needed: the main canvas and its channels are
# refcounted state, so the test runs headless.
add_executable(test_set_output_source_characterization test_set_output_source_characterization.c)
target_include_directories(test_set_output_source_characterization PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_set_output_source_characterization PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_set_output_source_characterization)
add_test(NAME test_set_output_source_characterization COMMAND test_set_output_source_characterization)
set_tests_properties(test_set_output_source_characterization PROPERTIES TIMEOUT 30 SKIP_RETURN_CODE 77)
