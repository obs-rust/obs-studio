# A recording to a URL (Custom Output (FFmpeg)) must not be stopped for low
# disk space while it runs (obsproject/obs-studio#12952). Builds the plain C
# check the frontend calls, so the test needs neither Qt nor a display.
add_executable(
  test_record_disk_space
  test_record_disk_space.c
  "${CMAKE_SOURCE_DIR}/frontend/utility/record-disk-space.c"
)
target_include_directories(test_record_disk_space PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_record_disk_space PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_record_disk_space)
add_test(test_record_disk_space ${CMAKE_CURRENT_BINARY_DIR}/test_record_disk_space)
