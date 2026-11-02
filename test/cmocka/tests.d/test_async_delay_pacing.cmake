# Timestamp-driven Video Delay queue regression (#3368); no video or sleeps.
add_executable(test_async_delay_pacing test_async_delay_pacing.c)
target_include_directories(test_async_delay_pacing PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}")
target_link_libraries(test_async_delay_pacing PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_async_delay_pacing)
add_test(test_async_delay_pacing ${CMAKE_CURRENT_BINARY_DIR}/test_async_delay_pacing)
