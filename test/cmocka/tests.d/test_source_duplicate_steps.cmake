# Characterization of the duplicate decisions of obs_source_duplicate(),
# get_new_filter_name(), obs_scene_duplicate(), dup_child() and dup_group(),
# moved to libobs/obs-source-duplicate.c before their port: the route of
# obs_source_duplicate() one step at a time, the filter numbers, the flags and
# group override of obs_scene_duplicate(), its per-item route, the reuse of a
# repeated source, the name of a copy and the group resize. Builds the moved
# file on its own: no display and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The real-libobs scene duplicate tests
# cover the callers.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the fifteen hidden functions, in the archive's obs-source member,
# which needs nothing else. It links OBS::libobs first (for obs.h's include
# directories), as test_scene_item_order_steps_rust does. Linking the archive
# directly avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_source_duplicate_steps_rust test_source_duplicate_steps.c)
  target_include_directories(
    test_source_duplicate_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_duplicate_steps_rust PRIVATE OBS::libobs libobs_rust-static ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_source_duplicate_steps_rust)
  add_test(test_source_duplicate_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_duplicate_steps_rust)
endif()
