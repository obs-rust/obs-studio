# Settings > Stream: picking "Show all..." in the Service list only expands the
# list. Builds the plain C helper the settings dialog calls, so the test needs
# neither Qt nor a display.
add_executable(
  test_stream_service_select
  test_stream_service_select.c
  "${CMAKE_SOURCE_DIR}/frontend/utility/stream-service-select.c"
)
target_include_directories(
  test_stream_service_select
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
)
target_link_libraries(test_stream_service_select PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_stream_service_select)
add_test(test_stream_service_select ${CMAKE_CURRENT_BINARY_DIR}/test_stream_service_select)
