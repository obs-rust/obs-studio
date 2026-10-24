# Characterization of the source and filter render routing of
# libobs/obs-source.c (source_render_video(), obs_source_main_render(),
# can_bypass(), obs_source_process_filter_begin_with_color_space(),
# obs_source_process_filter_tech_end() and obs_source_skip_video_filter()),
# moved to libobs/obs-source-render-route.c and ported to rust/obs-source
# (render_route): the target and parent check (#2177), the "no video", async
# update and inactive checks, every draw route with the values each one reads,
# the default-effect, sRGB and custom draw rules, the direct-render bypass for
# every flag, wiring and pair of colour spaces, the skip at zero size, the
# texrender format check and the technique name. Builds the moved file into
# itself. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. test_filter_color_space,
# test_filter_skip_removed and the other filter tests cover the callers through
# the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the 23 hidden functions and includes their header and obs.h (for
# the enum obs_source_type, obs_allow_direct_render, gs_color_space and
# gs_color_format values and the OBS_SOURCE_* flags; obs.h pulls SIMDe and
# obsconfig.h, hence libobs's include directories), so it links the archive
# alone: its obs-source member needs nothing from libobs or the other crates.
# Linking the archive directly avoids exporting private symbols just for a
# test.
if(TARGET libobs_rust-static)
  add_executable(test_source_render_route_steps_rust test_source_render_route_steps.c)
  target_include_directories(
    test_source_render_route_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES> ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_render_route_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_source_render_route_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_source_render_route_steps_rust)
  add_test(test_source_render_route_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_render_route_steps_rust)
endif()
