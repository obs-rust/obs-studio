# Public loading paths pin the pre-port FFmpeg initialization behavior.
add_executable(test_ffmpeg_image_init "${CMAKE_CURRENT_SOURCE_DIR}/test_ffmpeg_image_init.c")
target_link_libraries(test_ffmpeg_image_init PRIVATE OBS::libobs OBS::native-test-cmocka)
target_compile_definitions(
  test_ffmpeg_image_init
  PRIVATE
    FFMPEG_IMAGE_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/data"
    FFMPEG_IMAGE_CORRUPT="${CMAKE_CURRENT_BINARY_DIR}/ffmpeg-image-init-corrupt.png"
)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/ffmpeg-image-init-corrupt.png" "corrupt image, not a PNG\n")
add_test(NAME test_ffmpeg_image_init COMMAND test_ffmpeg_image_init)
add_dependencies(cmocka-tests test_ffmpeg_image_init)
