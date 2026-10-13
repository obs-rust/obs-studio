# Image Source must apply the EXIF orientation of a JPEG, PNG or WebP (OBS
# forum thread 195973). Generates the images with libavcodec and decodes
# them through gs_create_texture_file_data(), the CPU path of the image
# source, so it needs no display or graphics module.
add_executable(test_image_exif_orientation test_image_exif_orientation.c)
target_include_directories(test_image_exif_orientation PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_image_exif_orientation
  PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avutil ${CMOCKA_LIBRARIES}
)
add_dependencies(cmocka-tests test_image_exif_orientation)
add_test(test_image_exif_orientation ${CMAKE_CURRENT_BINARY_DIR}/test_image_exif_orientation)
set_tests_properties(test_image_exif_orientation PROPERTIES TIMEOUT 60)
