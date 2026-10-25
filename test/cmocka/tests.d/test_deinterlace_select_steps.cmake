# Characterization of the deinterlace frame selection of first_frame(),
# ready_deinterlace_frames(), deinterlace_get_closest_frames() and
# deinterlace_update_async_video() in libobs/obs-source-deinterlace.c, moved to
# libobs/obs-source-deinterlace-select.c and ported to rust/obs-source
# (deinterlace_select): the first frame with one and two queued, the unbuffered
# drops and the "don't skip ahead" guard, the buffered walk (out of bounds,
# FRAME_TOLERANCE, the pairwise drop, the two-or-fewer exit and its #2009
# exception, the MAX_TS_VAR re-anchor) at its edges, for queues of 0 to 5, 30
# and 33 frames, the wraps, the take and the texture update. The callers are
# copied onto a stand-in source with every erase, release and prev_frame write
# recorded. Builds the moved file and obs-source-deinterlace-pacing.c, which it
# calls, into the test. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them.
# test_deinterlace_2x_field_order, test_media_restart_stale_frame and
# test_media_restart_first_frame cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the hidden functions of obs-source-deinterlace-select.c and
# obs-source-deinterlace-pacing.c and includes only their headers, so it links
# the archive alone: its obs-source member needs nothing from libobs or the
# other crates. Linking the archive directly avoids exporting private symbols
# just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_deinterlace_select_steps_rust test_deinterlace_select_steps.c)
  target_include_directories(
    test_deinterlace_select_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_deinterlace_select_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_deinterlace_select_steps_rust)
  add_test(test_deinterlace_select_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_deinterlace_select_steps_rust)
endif()
