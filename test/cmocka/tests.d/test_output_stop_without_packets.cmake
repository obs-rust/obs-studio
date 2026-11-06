# An output that was started but never received a packet must finish
# stopping, and destroying it must return (#3739). Needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_output_stop_without_packets test_output_stop_without_packets.c)
  target_compile_definitions(
    test_output_stop_without_packets
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_output_stop_without_packets PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_output_stop_without_packets PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_output_stop_without_packets libobs-opengl)
  add_dependencies(cmocka-tests test_output_stop_without_packets)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_output_stop_without_packets
    COMMAND test_output_stop_without_packets
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_output_stop_without_packets PROPERTIES TIMEOUT 120)
endif()
