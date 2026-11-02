# Characterization of the time of each audio sample a transition mixes,
# get_sample_time() of libobs/obs-source-transition.c, moved to
# libobs/obs-source-transition-sample-time.c before its port to
# rust/obs-source (transition_sample_time, #3481): sample 0 at the block's
# time, every sample of a block at 1 Hz to 192 kHz, the last sample at
# 44100, 48000 and 96000 Hz, the floored offset, manual mode, other modes,
# a block before, straddling and after the transition, a duration of 0,
# and the uint64_t sum that wraps. A sample rate of 0 is not covered: the C
# divides by zero. Builds the moved file and the clock it asks
# (libobs/obs-source-transition-clock.c) into itself. The functions are
# hidden in libobs, so this characterizes that C source; the test calls
# only them. test_transition_audio_rescale and
# test_group_hide_transition_audio cover the mix through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls the two hidden functions, which come from the archive's obs-source
# member. Linking the archive directly avoids exporting private symbols just
# for a test.
if(TARGET libobs_rust-static)
  add_executable(test_transition_sample_time_steps_rust test_transition_sample_time_steps.c)
  target_include_directories(
    test_transition_sample_time_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(
    test_transition_sample_time_steps_rust
    PRIVATE OBS::libobs libobs_rust-static ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_transition_sample_time_steps_rust)
  add_test(test_transition_sample_time_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_transition_sample_time_steps_rust)
endif()
