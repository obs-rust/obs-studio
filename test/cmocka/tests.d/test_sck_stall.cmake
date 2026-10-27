# SCK stall detection for the macOS video capture tick (the helper is plain C,
# so the Linux cmocka suite runs the decisions the macOS build uses).
add_executable(test_sck_stall test_sck_stall.c "${CMAKE_SOURCE_DIR}/plugins/mac-capture/mac-sck-stall.c.h")
target_include_directories(test_sck_stall PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/mac-capture")
target_link_libraries(test_sck_stall PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_sck_stall)
add_test(test_sck_stall ${CMAKE_CURRENT_BINARY_DIR}/test_sck_stall)
