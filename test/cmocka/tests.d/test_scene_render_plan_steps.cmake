# Characterization of the per-item render decisions of render_item() and
# render_item_texture() in libobs/obs-scene.c, moved to
# libobs/obs-scene-render-plan.c before their port: the texture triggers, the
# texrender action, the empty-source rule (#573), the clear colour (#1406),
# the child drawn, the linear sRGB state, centered texture coordinates (a byte
# compare: -0.0 and NaN bits), the effect from the scale filter and the output
# scale (0.5 and 1 +- EPSILON edges, NaN), and the SDR white multiplier and
# technique for every pair of colour spaces (white levels 80, 300 and 0).
# Builds the moved file on its own: no display and no libobs core, so it runs
# on every platform. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them (test_scene_render_plan_steps_rust
# runs it against the Rust shims). The real-libobs scene tests cover the
# caller.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the hidden functions and includes only their header (with
# graphics/matrix4.h), so it links the archive alone: its obs-scene member
# needs nothing from libobs or the other crates. Linking the archive directly
# avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_scene_render_plan_steps_rust test_scene_render_plan_steps.c)
  target_include_directories(
    test_scene_render_plan_steps_rust
    PRIVATE
      "${CMAKE_SOURCE_DIR}/libobs"
      ${CMOCKA_INCLUDE_DIR}
      # graphics/matrix4.h pulls in util/sse-intrin.h, which needs SIMDe: take
      # libobs's include directories without linking libobs itself.
      $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES>
  )
  target_link_libraries(test_scene_render_plan_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(UNIX)
    target_link_libraries(test_scene_render_plan_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_scene_render_plan_steps_rust)
  add_test(test_scene_render_plan_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_scene_render_plan_steps_rust)
endif()
