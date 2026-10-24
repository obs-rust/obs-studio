# Characterization of the size query routing of obs_source_get_width() and
# obs_source_get_height() in libobs/obs-source.c (get_async_width(),
# get_base_width(), get_recurse_width() and their height twins), moved to
# libobs/obs-source-size.c before its port: every route for every
# combination of the type, data, a size callback, a filter parent, enabled
# and async active, the axis swap for the rotations around 0-359 and the
# ends of long, and where a filter, an input, a transition and a scene start.
# Builds the moved file into itself. The functions are hidden in libobs, so
# this characterizes that C source; the test calls only them.
# test_slideshow_size, test_filter_color_space, test_scene_item_crop,
# test_scene_rotated_scale_filter and test_async_rotation_normalize cover the
# caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the two hidden functions and includes only their header (no
# SIMDe and no obsconfig.h), so it links the archive alone: its obs-source
# member needs nothing from libobs or the other crates. Linking the archive
# directly avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_source_size_route_steps_rust test_source_size_route_steps.c)
  target_include_directories(
    test_source_size_route_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_size_route_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_source_size_route_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_source_size_route_steps_rust)
  add_test(test_source_size_route_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_size_route_steps_rust)
endif()
