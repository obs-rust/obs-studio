# Characterization of the transform update decisions of scene_video_render()
# and update_transforms_and_prune_sources() in libobs/obs-scene.c, moved to
# libobs/obs-scene-transform-update.c before their port: the scene size step
# (the same size, a new width, height or both, swapped, back to 0 x 0, the
# limits, a run of renders) and whether an item's transform is computed
# again (each input alone, every pattern, a set flag with the 0 x 0 the
# caller passes, each dimension compared on its own). Builds the moved file
# on its own: no display and no libobs core, so it runs on every platform.
# The functions are hidden in libobs, so this characterizes that C source;
# the test calls only them. test_scene_transform_update_walk covers the
# caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the two hidden functions, in the archive's obs-scene member,
# which needs nothing else. It links OBS::libobs first, as
# test_scene_item_names_steps_rust does. Linking the archive directly avoids
# exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_scene_transform_update_steps_rust test_scene_transform_update_steps.c)
  target_include_directories(
    test_scene_transform_update_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(
    test_scene_transform_update_steps_rust
    PRIVATE OBS::libobs libobs_rust-static ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_scene_transform_update_steps_rust)
  add_test(test_scene_transform_update_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_scene_transform_update_steps_rust)
endif()
