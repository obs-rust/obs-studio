# Characterization of the 2x deinterlace frame pacing of
# deinterlace_get_closest_frames() and ready_deinterlace_frames() in
# libobs/obs-source-deinterlace.c, moved to
# libobs/obs-source-deinterlace-pacing.c and ported to rust/obs-source
# (deinterlace_pacing): the unbuffered hold and its jump threshold, the "don't
# skip ahead" guard, the half duration, the buffered offset and the unbuffered
# re-anchor (#395), every branch, the thresholds at their edges, the wraps, and
# unbuffered sequences at 25, 29.97, 50 and 59.94 frames a second into 30 and
# 60 fps. Builds the moved file on its own: no display and no libobs core, so
# it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them.
# test_deinterlace_2x_field_order covers the caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the six hidden functions and includes only their header, so it
# links the archive alone: its obs-source member needs nothing from libobs or
# the other crates. Linking the archive directly avoids exporting private
# symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_deinterlace_pacing_steps_rust test_deinterlace_pacing_steps.c)
  target_include_directories(
    test_deinterlace_pacing_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_deinterlace_pacing_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_deinterlace_pacing_steps_rust)
  add_test(test_deinterlace_pacing_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_deinterlace_pacing_steps_rust)
endif()
