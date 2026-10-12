# A Stinger transition must play its whole clip when the clip's header gives
# a short duration or none (upstream obsproject/obs-studio#9755). Loads the
# built obs-transitions module and builds obs-ffmpeg-source.c directly, so
# the obs-ffmpeg module and its hardware encoder probes are not loaded.
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
if(OS_LINUX AND TARGET obs-transitions AND TARGET OBS::libobs-opengl AND TARGET OBS::media-playback)
  add_executable(
    test_stinger_media_duration
    test_stinger_media_duration.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-source.c"
  )
  target_compile_definitions(
    test_stinger_media_duration
    PRIVATE
      GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
      TRANSITIONS_MODULE_PATH="$<TARGET_FILE:obs-transitions>"
      TRANSITIONS_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/obs-transitions/data"
  )
  target_include_directories(
    test_stinger_media_duration
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_link_libraries(
    test_stinger_media_duration
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
  add_dependencies(test_stinger_media_duration obs-transitions libobs-opengl)
  add_dependencies(cmocka-tests test_stinger_media_duration)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_stinger_media_duration
    COMMAND test_stinger_media_duration
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_stinger_media_duration PROPERTIES TIMEOUT 120)
endif()
