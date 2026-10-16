# A Media Source playing a WebM with alpha from a URL showed no video: the
# keyframe gate for network inputs waited for AV_FRAME_FLAG_KEY, which
# FFmpeg's libvpx decoders never set. Plays generated clips through
# media-playback directly, so it needs no display or graphics.
if(TARGET OBS::media-playback)
  add_executable(test_media_alpha_network test_media_alpha_network.c)
  target_include_directories(test_media_alpha_network PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_media_alpha_network
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
  add_dependencies(cmocka-tests test_media_alpha_network)
  add_test(test_media_alpha_network ${CMAKE_CURRENT_BINARY_DIR}/test_media_alpha_network)
  set_tests_properties(test_media_alpha_network PROPERTIES TIMEOUT 120)
endif()
