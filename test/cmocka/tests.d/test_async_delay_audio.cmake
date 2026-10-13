# A Video Delay (Async) filter of more than 2 s must not break up the audio of
# the source it delays, and the audio must stay delayed with the video
# (upstream obsproject/obs-studio#13830). Builds async-delay-filter.c directly
# so the obs-filters module is not loaded. Mixing needs core video, so it runs
# (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(
    test_async_delay_audio
    test_async_delay_audio.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-filters/async-delay-filter.c"
  )
  target_compile_definitions(test_async_delay_audio PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_async_delay_audio PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_async_delay_audio PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_async_delay_audio libobs-opengl)
  add_dependencies(cmocka-tests test_async_delay_audio)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_async_delay_audio COMMAND test_async_delay_audio WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_async_delay_audio PROPERTIES TIMEOUT 180)
endif()
