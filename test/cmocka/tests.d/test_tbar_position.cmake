# Studio Mode T-Bar driven through the frontend API (obs_frontend_set_tbar_position
# and obs_frontend_release_tbar, used by obs-websocket's SetTBarPosition).
# Builds the plain C helper the frontend calls, so the test needs neither Qt
# nor a display.
add_executable(test_tbar_position test_tbar_position.c "${CMAKE_SOURCE_DIR}/frontend/utility/tbar-position.c")
target_include_directories(test_tbar_position PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_tbar_position PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_tbar_position)
add_test(test_tbar_position ${CMAKE_CURRENT_BINARY_DIR}/test_tbar_position)
