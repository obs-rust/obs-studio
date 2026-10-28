# Display re-initialization after a display configuration change (the helper
# is plain C, so the Linux cmocka suite runs the decisions the macOS build
# uses; obs-rust/obs-studio#2838).
add_executable(test_display_reinit test_display_reinit.c "${CMAKE_SOURCE_DIR}/plugins/mac-capture/display-reinit.c.h")
target_include_directories(test_display_reinit PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/mac-capture")
target_link_libraries(test_display_reinit PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_display_reinit)
add_test(test_display_reinit ${CMAKE_CURRENT_BINARY_DIR}/test_display_reinit)
