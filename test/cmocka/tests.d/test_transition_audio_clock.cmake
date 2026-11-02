# A transition mixes each audio block with one copy of its clock (#3499).
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_transition_audio_clock test_transition_audio_clock.c)
  target_compile_definitions(
    test_transition_audio_clock
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_transition_audio_clock PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_transition_audio_clock PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_transition_audio_clock libobs-opengl)
  add_dependencies(cmocka-tests test_transition_audio_clock)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_transition_audio_clock
    COMMAND test_transition_audio_clock
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_transition_audio_clock PROPERTIES TIMEOUT 60)
endif()
