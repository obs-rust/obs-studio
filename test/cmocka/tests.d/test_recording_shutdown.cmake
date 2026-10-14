# Quitting OBS while recording stops the recording gracefully, within a
# bounded time, before the outputs are destroyed (forum thread 182904). Runs
# the frontend's StopRecordingForShutdown() on a real libobs output. Needs
# core video, so it runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_recording_shutdown test_recording_shutdown.cpp)
  target_compile_definitions(test_recording_shutdown PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(
    test_recording_shutdown
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/widgets"
  )
  target_link_libraries(test_recording_shutdown PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_recording_shutdown libobs-opengl)
  add_dependencies(cmocka-tests test_recording_shutdown)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_recording_shutdown COMMAND test_recording_shutdown WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_recording_shutdown PROPERTIES TIMEOUT 120)
endif()
