# Characterization of the audio input steps of process_audio() and
# reset_resampler() in libobs/obs-source.c, moved to
# libobs/obs-source-audio-input.c before its port: the resampler restart
# on a discontinuity (#546) at the threshold, both ways and with no end
# set; each reset trigger alone; the clock drift fix's "no resampler
# needed" rule; the balance gate at its edges, NaN and out-of-range
# balances; the Force Mono gate; the sine, square and linear laws bit for
# bit and an unknown law; the downmix of 2, 6 and 8 channels and of no
# frames. Builds the moved file into itself. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_source_audio_balance_mono, test_audio_restart_resampler_tail and
# test_audio_clock_drift cover the caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the seven hidden functions and includes only their header, so it
# links the archive alone: its obs-source member needs nothing from libobs or
# the other crates. Linking the archive directly avoids exporting private
# symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_source_audio_input_steps_rust test_source_audio_input_steps.c)
  target_include_directories(
    test_source_audio_input_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_audio_input_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_source_audio_input_steps_rust)
  add_test(test_source_audio_input_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_audio_input_steps_rust)
endif()
