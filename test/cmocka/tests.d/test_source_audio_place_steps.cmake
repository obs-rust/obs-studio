# Characterization of the source audio placement and action timeline in
# libobs/obs-source.c, moved to libobs/obs-source-audio-place.c and ported
# to rust/obs-source (audio_place): the reset with no audio_ts or an older
# packet, the size and byte placement at 0, one frame and one second for
# 8000, 44100, 48000 and 384000 Hz, the drop just under, at and over
# MAX_BUF_SIZE, the pop off the back, each monitoring type, the action
# frames at, before and after audio_ts and at AUDIO_OUTPUT_FRAMES - 1 and
# at it, the fill and the mixes, and copies of the callers on a stand-in
# source. Builds the moved file into itself. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_async_delay_audio, test_audio_sync_offset_change,
# test_audio_clock_drift, test_audio_buffering_subsample,
# test_media_sync_offset_restart, test_audio_restart_resampler_tail,
# test_source_volume_mix, test_source_audio_ranges and test_audio_controls
# cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the eleven hidden functions, and includes their header and obs.h
# (for the libobs values the header mirrors; obs.h pulls SIMDe and
# obsconfig.h, hence libobs's include directories), so it links the archive
# alone: its obs-source member needs nothing from libobs or the other crates.
# Linking the archive directly avoids exporting private symbols just for a
# test.
if(TARGET libobs_rust-static)
  add_executable(test_source_audio_place_steps_rust test_source_audio_place_steps.c)
  target_include_directories(
    test_source_audio_place_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" $<TARGET_PROPERTY:libobs,INTERFACE_INCLUDE_DIRECTORIES> ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_audio_place_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_source_audio_place_steps_rust)
  add_test(test_source_audio_place_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_audio_place_steps_rust)
endif()
