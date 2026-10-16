# A Media Source playing a file whose timestamps do not start at 0 (MPEG-TS)
# reported the media time with the start time added and seeked start-time
# early. Plays generated clips through media-playback directly, so it needs
# no display or graphics.
if(TARGET OBS::media-playback)
  add_executable(test_media_ts_start_time test_media_ts_start_time.c)
  target_include_directories(test_media_ts_start_time PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_media_ts_start_time
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
  add_dependencies(cmocka-tests test_media_ts_start_time)
  add_test(test_media_ts_start_time ${CMAKE_CURRENT_BINARY_DIR}/test_media_ts_start_time)
  set_tests_properties(test_media_ts_start_time PROPERTIES TIMEOUT 120)
endif()
