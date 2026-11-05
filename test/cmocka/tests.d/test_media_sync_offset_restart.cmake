# A Media Source's audio sync offset must survive a restart
# (upstream obsproject/obs-studio#11640). Builds obs-ffmpeg-source.c
# directly so the obs-ffmpeg module and its hardware encoder probes are not
# loaded. Needs core video, so it runs (under xvfb-run) on Linux and skips
# without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl AND TARGET OBS::media-playback)
  add_executable(
    test_media_sync_offset_restart
    test_media_sync_offset_restart.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-source.c.h"
  )
  target_compile_definitions(
    test_media_sync_offset_restart
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(
    test_media_sync_offset_restart
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_link_libraries(
    test_media_sync_offset_restart
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
  add_dependencies(test_media_sync_offset_restart libobs-opengl)
  add_dependencies(cmocka-tests test_media_sync_offset_restart)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_media_sync_offset_restart
    COMMAND test_media_sync_offset_restart
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  # Real-time media/audio offsets must not overlap the other timing tests (#3661).
  set_tests_properties(test_media_sync_offset_restart PROPERTIES TIMEOUT 120 RESOURCE_LOCK timing)
endif()
