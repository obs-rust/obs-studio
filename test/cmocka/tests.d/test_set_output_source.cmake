# Call the exported API from the selected real hybrid/C-reference libobs.
add_executable(test_set_output_source test_set_output_source.c)
target_include_directories(test_set_output_source PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_set_output_source PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(TARGET OBS::w32-pthreads)
  target_link_libraries(test_set_output_source PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_set_output_source)
if(XVFB_RUN)
  add_test(
    NAME test_set_output_source
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_set_output_source>
  )
else()
  add_test(NAME test_set_output_source COMMAND test_set_output_source)
endif()
set_tests_properties(test_set_output_source PROPERTIES TIMEOUT 30)
