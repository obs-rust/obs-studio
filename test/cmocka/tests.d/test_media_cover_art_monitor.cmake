# Audio monitoring of a Media Source with embedded cover art after the
# source restarts (upstream obsproject/obs-studio#9428, #11739): the
# monitor's video sync decision, fed by the real ffmpeg_source. Builds
# obs-ffmpeg-source.c directly so the obs-ffmpeg module and its hardware
# encoder probes are not loaded. Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl AND TARGET OBS::media-playback)
  add_executable(
    test_media_cover_art_monitor
    test_media_cover_art_monitor.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-source.c.h"
  )
  target_compile_definitions(
    test_media_cover_art_monitor
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(
    test_media_cover_art_monitor
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg" "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring"
  )
  target_link_libraries(
    test_media_cover_art_monitor
    PRIVATE
      OBS::libobs
      OBS::caption
      OBS::media-playback
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      FFmpeg::swscale
      FFmpeg::swresample
      ${CMOCKA_LIBRARIES}
      m
  )
  add_dependencies(test_media_cover_art_monitor libobs-opengl)
  add_dependencies(cmocka-tests test_media_cover_art_monitor)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_media_cover_art_monitor
    COMMAND test_media_cover_art_monitor
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_media_cover_art_monitor PROPERTIES TIMEOUT 120)
endif()
