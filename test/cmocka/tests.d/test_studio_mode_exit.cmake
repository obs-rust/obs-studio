# Scene switch when Studio Mode is turned off (obsproject/obs-websocket#1150).
# Builds the plain C helper the frontend calls, so the test needs neither Qt
# nor a display.
add_executable(test_studio_mode_exit test_studio_mode_exit.c "${CMAKE_SOURCE_DIR}/frontend/utility/studio-mode-exit.c")
target_include_directories(test_studio_mode_exit PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_studio_mode_exit PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_studio_mode_exit)
add_test(test_studio_mode_exit ${CMAKE_CURRENT_BINARY_DIR}/test_studio_mode_exit)
