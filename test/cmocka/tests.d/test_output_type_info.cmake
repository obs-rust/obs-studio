# Actual-library output-type registry characterization, shared by both libobs modes.
add_executable(test_output_type_info test_output_type_info.c)
target_link_libraries(test_output_type_info PRIVATE OBS::libobs OBS::native-test-cmocka)
add_dependencies(cmocka-tests test_output_type_info)
if(XVFB_RUN)
  add_test(
    NAME test_output_type_info
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_output_type_info>
  )
else()
  add_test(NAME test_output_type_info COMMAND test_output_type_info)
endif()
set_tests_properties(test_output_type_info PROPERTIES TIMEOUT 30)
