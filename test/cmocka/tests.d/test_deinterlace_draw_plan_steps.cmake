# Characterization of the deinterlace draw decisions of
# libobs/obs-source-deinterlace.c (get_effect(), deinterlace_linear_required(),
# deinterlace_render() and obs_source_set_deinterlace_mode()), moved to
# libobs/obs-source-deinterlace-draw.c and ported to rust/obs-source
# (deinterlace_draw): the effect and the linear rule of every mode and of
# modes outside the enum, the early return, the technique and multiplier of
# every pair of colour spaces at SDR white levels of 80, 300 and 0, the second
# field at its threshold and where the uint64_t sum wraps, and every mode
# change (#2120). Builds the moved file into itself. The functions
# are hidden in libobs, so this characterizes that C source; the test calls
# only them. test_deinterlace_2x_field_order and test_deinterlace_mode_load
# cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the ten hidden functions and includes their header and obs.h
# (for the enum obs_deinterlace_mode and enum gs_color_space values; obs.h
# pulls SIMDe and obsconfig.h, hence libobs's include directories), so it
# links the archive alone: its obs-source member needs nothing from libobs or
# the other crates. Linking the archive directly avoids exporting private
# symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_deinterlace_draw_plan_steps_rust test_deinterlace_draw_plan_steps.c)
  target_include_directories(
    test_deinterlace_draw_plan_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES> ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_deinterlace_draw_plan_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_deinterlace_draw_plan_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_deinterlace_draw_plan_steps_rust)
  add_test(test_deinterlace_draw_plan_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_deinterlace_draw_plan_steps_rust)
endif()
