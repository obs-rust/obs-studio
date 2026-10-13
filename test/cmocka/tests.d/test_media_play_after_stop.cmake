# A stopped or ended Media Source must play again on Play (upstream
# obsproject/obs-websocket#1046, obsproject/obs-studio#4252). Builds
# obs-ffmpeg-source.c directly so the obs-ffmpeg module and its hardware
# encoder probes are not loaded. Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl AND TARGET OBS::media-playback)
  add_executable(
    test_media_play_after_stop
    test_media_play_after_stop.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-source.c"
  )
  target_compile_definitions(
    test_media_play_after_stop
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(
    test_media_play_after_stop
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_link_libraries(
    test_media_play_after_stop
    PRIVATE
      OBS::libobs
      OBS::media-playback
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      FFmpeg::swscale
      FFmpeg::swresample
      ${CMOCKA_LIBRARIES}
      m
  )
  add_dependencies(test_media_play_after_stop libobs-opengl)
  add_dependencies(cmocka-tests test_media_play_after_stop)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_media_play_after_stop
    COMMAND test_media_play_after_stop
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_media_play_after_stop PROPERTIES TIMEOUT 60)
endif()
