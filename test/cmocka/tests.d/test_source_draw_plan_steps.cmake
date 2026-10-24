# Characterization of the source draw decisions of libobs/obs-source.c
# (rotate_async_video(), obs_source_render_async_video() and source_render()),
# moved to libobs/obs-source-draw-plan.c and ported to rust/obs-source
# (draw_plan): the rotation translate for 0, 90, 180, 270 and other angles on
# non-square sizes, the linear sRGB flag and the non-linear alpha blend of an
# async frame, the technique of every pair of colour spaces with and without
# non-linear alpha, the white level read and the multiplier at SDR white
# levels of 80, 300 and 0, and whether source_render() converts through
# color_space_texrender. Builds the moved file into itself. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. test_async_rotation_normalize, test_filter_color_space and
# test_source_color_space_steps cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the eight hidden functions and the header's
# obs_source_draw_technique_name(), and includes their header and obs.h (for
# the enum gs_color_space values; obs.h pulls SIMDe and obsconfig.h, hence
# libobs's include directories), so it links the archive alone: its
# obs-source member needs nothing from libobs or the other crates. Linking the
# archive directly avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_source_draw_plan_steps_rust test_source_draw_plan_steps.c)
  target_include_directories(
    test_source_draw_plan_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES> ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_draw_plan_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_source_draw_plan_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_source_draw_plan_steps_rust)
  add_test(test_source_draw_plan_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_draw_plan_steps_rust)
endif()
