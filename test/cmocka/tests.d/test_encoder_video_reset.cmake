# Video encoders follow obs_reset_video() instead of keeping the freed video_t
# (obsproject/obs-studio#12213). Needs core video, so it runs (under xvfb-run)
# on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_encoder_video_reset test_encoder_video_reset.c)
  target_compile_definitions(test_encoder_video_reset PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_encoder_video_reset PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_encoder_video_reset PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_encoder_video_reset libobs-opengl)
  add_dependencies(cmocka-tests test_encoder_video_reset)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_encoder_video_reset
    COMMAND test_encoder_video_reset
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_encoder_video_reset PROPERTIES TIMEOUT 120)
endif()
