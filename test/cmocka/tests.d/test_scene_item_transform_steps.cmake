# Characterization of the scene item transform composition of
# compute_item_transform() in libobs/obs-scene.c, moved to
# libobs/obs-scene-item-transform.c before its port: the bounds crop reset,
# a nested scene's crop rescaled by its size change (last_width 0, a size
# change, the same size, truncation, saturation), the cropped size, the
# bounds test, the box scale and both matrices (every alignment and two
# with extra bits, each bounds type, crop on and off, absolute and
# relative, the four scale signs, rotations 0, 90, 45, -30 and 720), run in
# compute_item_transform()'s order. Flipped items are pinned through
# x86-64's float-to-uint32_t cast, done in defined C, so the pins hold on
# every architecture. Builds the moved file into itself, with the hidden
# crop, coordinate and bounds files it is composed with; matrix4 comes from
# libobs. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The real-libobs scene, group and
# clipboard tests cover the caller.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls the four hidden functions, and through compose() the hidden crop,
# coordinate, bounds and alignment functions, all in the archive's obs-scene
# member; matrix4_* are libobs's exports. It links OBS::libobs first, so
# libobs gives matrix4_* and the archive only the hidden functions, as
# test_scene_item_names_steps_rust does. Linking the archive directly avoids
# exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_scene_item_transform_steps_rust test_scene_item_transform_steps.c)
  target_include_directories(
    test_scene_item_transform_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_scene_item_transform_steps_rust PRIVATE OBS::libobs libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_scene_item_transform_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_scene_item_transform_steps_rust)
  add_test(test_scene_item_transform_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_scene_item_transform_steps_rust)
endif()
