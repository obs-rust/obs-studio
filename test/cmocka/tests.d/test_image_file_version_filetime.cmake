# The Image source's file check on Windows (follow-up to
# obsproject/obs-studio#10700): the conversion of a Windows file time, tested
# on every platform, and on Windows the code that reads it.
add_executable(
  test_image_file_version_filetime
  test_image_file_version_filetime.c
  "${CMAKE_SOURCE_DIR}/plugins/image-source/image-file-version.c"
)
target_include_directories(
  test_image_file_version_filetime
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/image-source"
)
if(OS_WINDOWS)
  target_sources(
    test_image_file_version_filetime
    PRIVATE "${CMAKE_SOURCE_DIR}/plugins/image-source/image-file-version-win32.c"
  )
endif()
target_link_libraries(test_image_file_version_filetime PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_image_file_version_filetime)
add_test(test_image_file_version_filetime ${CMAKE_CURRENT_BINARY_DIR}/test_image_file_version_filetime)
