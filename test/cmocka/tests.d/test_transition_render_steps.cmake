# Characterization of the render and audio step decisions of the transitions
# in libobs/obs-source-transition.c, moved to
# libobs/obs-source-transition-render.c before its port: the video stop step
# of obs_transition_video_render2() and obs_transition_video_render_direct()
# at t just below, at and above 1 and NaN, with the direct render's inactive
# rule; render2's draw choice and the direct render's child for every flag
# and target; the audio stop step with and without a timestamp, for children
# there, removed, pending or without audio; the mix or copy; the checks and
# start frame of transition_process_audio() at and around min_ts, at the
# AUDIO_OUTPUT_FRAMES edge and across the uint64_t wrap; and the offsets of
# recalculate_transition_matrix() for each scale type, with the product's
# rounding. Builds the moved file into itself. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_transition_swap, test_studio_mode_swap, test_transition_scale_type,
# test_transition_recursive_destination, test_item_transition_empty_source,
# test_saved_transition, test_fade_to_color_mix,
# test_transition_audio_rescale and test_group_hide_transition_audio cover
# the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the nine hidden functions and includes only their header (no
# obs.h, so no SIMDe), so it links the archive alone: its obs-source member
# needs nothing from libobs or the other crates. Linking the archive directly
# avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_transition_render_steps_rust test_transition_render_steps.c)
  target_include_directories(
    test_transition_render_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_transition_render_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_transition_render_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_transition_render_steps_rust)
  add_test(test_transition_render_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_transition_render_steps_rust)
endif()
