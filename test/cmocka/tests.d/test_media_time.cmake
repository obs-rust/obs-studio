# Media Source controls time label (obsproject/obs-studio#11183). Builds the
# plain C helper the frontend calls, so the test needs neither Qt nor a
# display.
add_executable(test_media_time test_media_time.c "${CMAKE_SOURCE_DIR}/frontend/utility/media-time.c")
target_include_directories(test_media_time PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_media_time PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_media_time)
add_test(test_media_time ${CMAKE_CURRENT_BINARY_DIR}/test_media_time)
