# obs_reset_video() for every output format with a GPU conversion (I420,
# NV12, I444, I010, P010, P216, P416, with their color spaces and both
# ranges) and BGRA/RGBA, through the real libobs and its OpenGL renderer:
# the frames' planes and samples, and the size check and alignment. Public
# API only, so it also covers the hybrid libobs. Needs core video, so it runs
# (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_video_output_formats test_video_output_formats.c)
  target_compile_definitions(test_video_output_formats PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_video_output_formats PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_video_output_formats PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_video_output_formats libobs-opengl)
  add_dependencies(cmocka-tests test_video_output_formats)
  # libobs finds its effect files relative to the working directory.
  find_program(XVFB_RUN xvfb-run)
  if(XVFB_RUN)
    add_test(
      NAME test_video_output_formats
      COMMAND ${XVFB_RUN} -a $<TARGET_FILE:test_video_output_formats>
      WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
    )
  else()
    add_test(
      NAME test_video_output_formats
      COMMAND test_video_output_formats
      WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
    )
  endif()
  set_tests_properties(test_video_output_formats PROPERTIES TIMEOUT 180)
endif()
