# Stopping an output must not change an encoder's packet timing array while the
# video thread can still be encoding (obs-rust/obs-studio#3788). Reads the array
# through obs-internal.h on the video thread. Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_encoder_stop_packet_times test_encoder_stop_packet_times.c)
  target_compile_definitions(
    test_encoder_stop_packet_times
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_encoder_stop_packet_times PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_encoder_stop_packet_times PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(test_encoder_stop_packet_times libobs-opengl)
  add_dependencies(cmocka-tests test_encoder_stop_packet_times)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_encoder_stop_packet_times
    COMMAND test_encoder_stop_packet_times
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_encoder_stop_packet_times PROPERTIES TIMEOUT 60)
endif()
