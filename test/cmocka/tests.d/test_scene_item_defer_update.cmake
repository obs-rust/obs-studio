# Actual-library characterization of obs_sceneitem_defer_update_begin/end (#4646) in both Tier 2 modes.
add_executable(test_scene_item_defer_update test_scene_item_defer_update.c)
target_link_libraries(test_scene_item_defer_update PRIVATE OBS::libobs OBS::native-test-cmocka)
if(OS_WINDOWS)
  target_link_libraries(test_scene_item_defer_update PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_scene_item_defer_update)
add_test(NAME test_scene_item_defer_update COMMAND test_scene_item_defer_update)
