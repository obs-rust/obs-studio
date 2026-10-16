# Stopping a Custom FFmpeg Output lost the frames its encoders held back
# (B-frames, lookahead) and the packets not yet written. Builds the real
# obs-ffmpeg-output.c into the test and wraps avcodec_send_frame() with the
# GNU linker's --wrap to count the frames encoded. Needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(
    test_ffmpeg_output_drain
    test_ffmpeg_output_drain.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-output.c"
  )
  target_compile_definitions(test_ffmpeg_output_drain PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(
    test_ffmpeg_output_drain
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_link_options(test_ffmpeg_output_drain PRIVATE -Wl,--wrap=avcodec_send_frame)
  target_link_libraries(
    test_ffmpeg_output_drain
    PRIVATE
      OBS::libobs
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      FFmpeg::swscale
      FFmpeg::swresample
      ${CMOCKA_LIBRARIES}
  )
  add_dependencies(test_ffmpeg_output_drain libobs-opengl)
  add_dependencies(cmocka-tests test_ffmpeg_output_drain)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_ffmpeg_output_drain
    COMMAND test_ffmpeg_output_drain
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_ffmpeg_output_drain PROPERTIES TIMEOUT 60)
endif()
