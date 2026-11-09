# Public CPU image loading against whichever real libobs the harness selects.
add_executable(test_ffmpeg_image_decode test_ffmpeg_image_decode.c)
target_include_directories(test_ffmpeg_image_decode PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_ffmpeg_image_decode
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash FFmpeg::avcodec FFmpeg::avutil ${CMOCKA_LIBRARIES}
)
if(OS_WINDOWS)
  target_link_libraries(test_ffmpeg_image_decode PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_ffmpeg_image_decode)
add_test(NAME test_ffmpeg_image_decode COMMAND test_ffmpeg_image_decode)
set_tests_properties(test_ffmpeg_image_decode PROPERTIES TIMEOUT 20)
