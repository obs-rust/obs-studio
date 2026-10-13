# An Image source must load its file again when it is replaced within the
# same second (obsproject/obs-studio#10700). Builds the plain C helper the
# source calls; needs neither graphics nor a display. Uses POSIX utimensat().
if(NOT OS_WINDOWS)
  add_executable(
    test_image_file_version
    test_image_file_version.c
    "${CMAKE_SOURCE_DIR}/plugins/image-source/image-file-version.c"
  )
  target_include_directories(
    test_image_file_version
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/image-source"
  )
  target_link_libraries(test_image_file_version PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_image_file_version)
  add_test(test_image_file_version ${CMAKE_CURRENT_BINARY_DIR}/test_image_file_version)
endif()
