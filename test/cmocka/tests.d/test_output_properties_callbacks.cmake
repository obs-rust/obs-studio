# Direct callback-data and applied-settings characterization, shared by both libobs modes.
add_executable(test_output_properties_callbacks test_output_properties_callbacks.c)
target_link_libraries(test_output_properties_callbacks PRIVATE OBS::libobs OBS::native-test-cmocka)
add_dependencies(cmocka-tests test_output_properties_callbacks)
add_test(NAME test_output_properties_callbacks COMMAND test_output_properties_callbacks)
