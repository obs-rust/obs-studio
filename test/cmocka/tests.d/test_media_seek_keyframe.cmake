# A Media Source seek must land on the requested time, not on the keyframe
# before it (obsproject/obs-websocket#1190). Plays generated clips through
# media-playback directly, so it needs no display or graphics.
if(TARGET OBS::media-playback)
  add_executable(test_media_seek_keyframe test_media_seek_keyframe.c)
  target_include_directories(test_media_seek_keyframe PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_media_seek_keyframe
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
  add_dependencies(cmocka-tests test_media_seek_keyframe)
  add_test(test_media_seek_keyframe ${CMAKE_CURRENT_BINARY_DIR}/test_media_seek_keyframe)
  set_tests_properties(test_media_seek_keyframe PROPERTIES TIMEOUT 120)
endif()
