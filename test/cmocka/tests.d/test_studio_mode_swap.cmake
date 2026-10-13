# Studio Mode swap after a transition (obsproject/obs-websocket#1320).
# Builds the plain C helper the frontend calls, so the test needs neither Qt
# nor a display.
add_executable(test_studio_mode_swap test_studio_mode_swap.c "${CMAKE_SOURCE_DIR}/frontend/utility/studio-mode-swap.c")
target_include_directories(test_studio_mode_swap PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_studio_mode_swap PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_studio_mode_swap)
add_test(test_studio_mode_swap ${CMAKE_CURRENT_BINARY_DIR}/test_studio_mode_swap)
