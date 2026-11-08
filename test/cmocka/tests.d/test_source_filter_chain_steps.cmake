# Characterization of the filter chain decisions of filter_compatible(),
# obs_source_filter_add(), obs_source_filter_remove_refless(),
# move_filter_dir() with find_next_filter() and find_prev_filter(),
# reorder_filter_targets() and obs_source_restore_filters(), moved to
# libobs/obs-source-filter-chain.c before their port: compatibility, the add
# gates and target, the remove retarget, the movements and walks, the target
# chain, and the restore name match and load. Builds the moved file on its
# own: no display and no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. The real-libobs filter tests cover the callers.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the thirteen hidden functions, in the archive's obs-source member,
# which needs nothing else. It links OBS::libobs first (for obs.h's include
# directories), as test_source_duplicate_steps_rust does. Linking the archive
# directly avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_source_filter_chain_steps_rust test_source_filter_chain_steps.c)
  target_include_directories(
    test_source_filter_chain_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_filter_chain_steps_rust PRIVATE OBS::libobs libobs_rust-static ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_source_filter_chain_steps_rust)
  add_test(test_source_filter_chain_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_filter_chain_steps_rust)
endif()
