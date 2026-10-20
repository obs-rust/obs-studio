# A delayed output that stopped itself without a reconnect (a lost connection
# with auto-reconnect off, an encoder error) did not start normally again: no
# "start" signal, and delayed packets reached it before it connected. Real
# libobs with core video, so it runs under xvfb-run on Linux and skips without
# a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_output_delay_restart test_output_delay_restart.c)
  target_compile_definitions(test_output_delay_restart PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_output_delay_restart PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_output_delay_restart PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(test_output_delay_restart libobs-opengl)
  add_dependencies(cmocka-tests test_output_delay_restart)
  add_test(
    NAME test_output_delay_restart
    COMMAND test_output_delay_restart
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_output_delay_restart PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
endif()
