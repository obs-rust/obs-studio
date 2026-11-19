# This executable must link without libobs or a graphics backend. Keep the
# atomic state and callback together in the actual production unity member.
add_executable(
  test_image_max_texture_size
  test_image_max_texture_size.c
  "${CMAKE_SOURCE_DIR}/libobs/build/libobs.graphics-image-max-size+.h.c"
)
target_include_directories(
  test_image_max_texture_size
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs"
)
target_link_libraries(test_image_max_texture_size PRIVATE ${CMOCKA_LIBRARIES} SIMDe::SIMDe Threads::Threads)
add_dependencies(cmocka-tests test_image_max_texture_size)
add_test(NAME test_image_max_texture_size COMMAND test_image_max_texture_size)
