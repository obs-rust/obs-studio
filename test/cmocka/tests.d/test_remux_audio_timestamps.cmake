# Remuxing an MKV to MP4/MOV keeps AAC frames sample accurate (upstream
# obsproject/obs-studio#13249). Generates the MKV with FFmpeg's AAC encoder and
# remuxes it through libobs' media_remux; needs no display.
add_executable(test_remux_audio_timestamps test_remux_audio_timestamps.c)
target_include_directories(test_remux_audio_timestamps PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_remux_audio_timestamps
  PRIVATE
    OBS::libobs
    FFmpeg::avcodec
    FFmpeg::avformat
    FFmpeg::avutil
    ${CMOCKA_LIBRARIES}
    $<$<NOT:$<PLATFORM_ID:Windows>>:m>
)
add_dependencies(cmocka-tests test_remux_audio_timestamps)
add_test(test_remux_audio_timestamps ${CMAKE_CURRENT_BINARY_DIR}/test_remux_audio_timestamps)
set_tests_properties(test_remux_audio_timestamps PROPERTIES TIMEOUT 60)
