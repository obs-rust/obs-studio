# Field-only characterization calls the actual library in both Tier 2 modes.
add_executable(test_scene_item_crop_setter test_scene_item_crop_setter.c)
target_include_directories(test_scene_item_crop_setter PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_scene_item_crop_setter PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_scene_item_crop_setter)
add_test(NAME test_scene_item_crop_setter COMMAND test_scene_item_crop_setter)
