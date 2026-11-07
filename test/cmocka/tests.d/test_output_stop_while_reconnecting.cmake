# An output that is stopped while it waits to reconnect must finish
# stopping, and destroying it must return (#3739). Needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_output_stop_while_reconnecting test_output_stop_while_reconnecting.c)
  target_compile_definitions(
    test_output_stop_while_reconnecting
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_output_stop_while_reconnecting PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_output_stop_while_reconnecting PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_output_stop_while_reconnecting libobs-opengl)
  add_dependencies(cmocka-tests test_output_stop_while_reconnecting)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_output_stop_while_reconnecting
    COMMAND test_output_stop_while_reconnecting
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_output_stop_while_reconnecting PROPERTIES TIMEOUT 120)
endif()
