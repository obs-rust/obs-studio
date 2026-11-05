# Placement half of calculate_bounds_data (alignment, crop, flip).
# The C-reference target/test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust internal symbols.
# The regular executable above remains the original C reference. Linking the
# archive directly avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_scene_bounds_place_rust test_scene_bounds_place.c)
  target_include_directories(test_scene_bounds_place_rust PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_bounds_place_rust PRIVATE libobs_rust-static OBS::libobs ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_scene_bounds_place_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_scene_bounds_place_rust)
  add_test(test_scene_bounds_place_rust ${CMAKE_CURRENT_BINARY_DIR}/test_scene_bounds_place_rust)
endif()
