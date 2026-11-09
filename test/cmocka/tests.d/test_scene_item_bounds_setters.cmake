# Actual-library characterization, exercised against hybrid and C reference by Tier 2.
add_executable(test_scene_item_bounds_setters test_scene_item_bounds_setters.c)
target_link_libraries(test_scene_item_bounds_setters PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_scene_item_bounds_setters PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_scene_item_bounds_setters)
add_test(NAME test_scene_item_bounds_setters COMMAND test_scene_item_bounds_setters)
