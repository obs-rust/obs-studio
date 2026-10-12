# Stopping a Custom FFmpeg Output freed its encoders while the video or
# audio thread could still be encoding with them
# (obsproject/obs-studio#7536). Builds the real obs-ffmpeg-output.c into the
# test and wraps two libavcodec calls with the GNU linker's --wrap. Needs
# core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(
    test_ffmpeg_output_stop
    test_ffmpeg_output_stop.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-output.c"
  )
  target_compile_definitions(test_ffmpeg_output_stop PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(
    test_ffmpeg_output_stop
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_link_options(test_ffmpeg_output_stop PRIVATE -Wl,--wrap=avcodec_send_frame -Wl,--wrap=avcodec_free_context)
  target_link_libraries(
    test_ffmpeg_output_stop
    PRIVATE
      OBS::libobs
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      FFmpeg::swscale
      FFmpeg::swresample
      ${CMOCKA_LIBRARIES}
  )
  add_dependencies(test_ffmpeg_output_stop libobs-opengl)
  add_dependencies(cmocka-tests test_ffmpeg_output_stop)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_ffmpeg_output_stop COMMAND test_ffmpeg_output_stop WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_ffmpeg_output_stop PROPERTIES TIMEOUT 60)
endif()
