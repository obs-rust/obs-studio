# A Media Source must follow a stream whose frame size or pixel format
# changes mid-file (upstream obsproject/obs-studio PR #5010). Plays generated
# clips through media-playback directly, so it needs no display or graphics.
if(TARGET OBS::media-playback)
  add_executable(test_media_format_change test_media_format_change.c)
  target_include_directories(test_media_format_change PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_media_format_change
    PRIVATE
      OBS::libobs
      OBS::media-playback
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      FFmpeg::swscale
      FFmpeg::swresample
      ${CMOCKA_LIBRARIES}
      $<$<NOT:$<PLATFORM_ID:Windows>>:m>
  )
  add_dependencies(cmocka-tests test_media_format_change)
  add_test(test_media_format_change ${CMAKE_CURRENT_BINARY_DIR}/test_media_format_change)
  set_tests_properties(test_media_format_change PROPERTIES TIMEOUT 120)
endif()
