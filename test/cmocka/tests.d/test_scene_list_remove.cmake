# Scene list events when a scene is removed (obsproject/obs-websocket#979).
# Builds the plain C helper the frontend calls, so the test needs neither Qt
# nor a display.
add_executable(
  test_scene_list_remove
  test_scene_list_remove.c
  "${CMAKE_SOURCE_DIR}/frontend/utility/scene-list-remove.c"
)
target_include_directories(test_scene_list_remove PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility")
target_link_libraries(test_scene_list_remove PRIVATE ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_scene_list_remove)
add_test(test_scene_list_remove ${CMAKE_CURRENT_BINARY_DIR}/test_scene_list_remove)
