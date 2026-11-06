# An async source whose audio runs on a clock of its own, hours from its video
# (NDI Test Patterns through DistroAV), must be heard on the program (#3747).
# Mixing needs core video, so it runs (under xvfb-run) on Linux and skips
# without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_async_audio_own_clock test_async_audio_own_clock.c)
  target_compile_definitions(
    test_async_audio_own_clock
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_async_audio_own_clock PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_async_audio_own_clock PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_async_audio_own_clock libobs-opengl)
  add_dependencies(cmocka-tests test_async_audio_own_clock)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_async_audio_own_clock
    COMMAND test_async_audio_own_clock
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  # Its checks depend on wall-clock timing: the asan pool runs it alone (#3238).
  set_tests_properties(test_async_audio_own_clock PROPERTIES TIMEOUT 180 RESOURCE_LOCK timing)
endif()
