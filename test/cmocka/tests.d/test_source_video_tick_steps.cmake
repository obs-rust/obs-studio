# Characterization of the per-frame decisions of obs_source_video_tick() and
# async_tick() in libobs/obs-source.c, moved to libobs/obs-source-video-tick.c
# and ported to rust/obs-source (video_tick): the transition tick for every
# source type in the enum and out, the async tick and the media actions for
# every output flag bit, each type with each combination of the flags, the
# deferred update for counts around 0 and at the ends of long, the show/hide
# and activate/deactivate edges in both directions and none for every count,
# a run of ticks, the callback rule and the async route, and a copy of the
# tick's order of steps on a stand-in source (the activate refs loaded after
# the show edge, the filters walked in reverse). Builds the moved file into
# itself. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. test_visibility_transition,
# test_preview_only_hide, test_studio_mode_swap, test_studio_mode_exit,
# test_canvas_destroy_scene_refs and test_item_transition_empty_source cover
# the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the seven hidden functions, and includes their header and obs.h
# (for the libobs values the header mirrors; obs.h pulls SIMDe and
# obsconfig.h, hence libobs's include directories), so it links the archive
# alone: its obs-source member needs nothing from libobs or the other crates.
# Linking the archive directly avoids exporting private symbols just for a
# test.
if(TARGET libobs_rust-static)
  add_executable(test_source_video_tick_steps_rust test_source_video_tick_steps.c)
  target_include_directories(
    test_source_video_tick_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES> ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_video_tick_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_source_video_tick_steps_rust)
  add_test(test_source_video_tick_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_video_tick_steps_rust)
endif()
