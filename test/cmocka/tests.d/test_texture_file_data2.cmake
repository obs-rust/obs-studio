# Pin the exported legacy image-loader wrapper against both libobs libraries.
add_executable(test_texture_file_data2 "${CMAKE_CURRENT_SOURCE_DIR}/test_texture_file_data2.c")
target_link_libraries(test_texture_file_data2 PRIVATE OBS::libobs OBS::native-test-cmocka)
target_compile_definitions(
  test_texture_file_data2
  PRIVATE
    TEXTURE_DATA2_DATA_DIR="${CMAKE_CURRENT_SOURCE_DIR}/data"
    TEXTURE_DATA2_CORRUPT="${CMAKE_CURRENT_BINARY_DIR}/texture-file-data2-corrupt.png"
)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/texture-file-data2-corrupt.png" "corrupt image, not a PNG\n")
add_test(NAME test_texture_file_data2 COMMAND test_texture_file_data2)
add_dependencies(cmocka-tests test_texture_file_data2)
