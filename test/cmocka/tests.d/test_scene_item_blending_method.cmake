# Actual-library characterization, unchanged for the later Rust swap.
add_executable(test_scene_item_blending_method test_scene_item_blending_method.c)
target_include_directories(test_scene_item_blending_method PRIVATE "${CMAKE_SOURCE_DIR}/libobs")
target_link_libraries(test_scene_item_blending_method PRIVATE OBS::libobs OBS::native-test-cmocka)
add_test(NAME test_scene_item_blending_method COMMAND test_scene_item_blending_method)
add_dependencies(cmocka-tests test_scene_item_blending_method)
