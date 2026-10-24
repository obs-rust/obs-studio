# Characterization of the audio render mix plan of obs_source_audio_render(),
# custom_audio_render() and process_audio_source_tick() in libobs/obs-source.c,
# moved to libobs/obs-source-audio-render.c and ported to rust/obs-source
# (audio_render_plan): every route (no buffer, a custom render with and without
# data, the tick, the submix tick), the clears before and after a custom
# render's callback for its success and failure, the tick's pending check at
# the size edges and with no timestamp, and the copy or clear of each mix over
# every pair of the six mix bits and the mask edges (0, 1, 0x3f, 0x40 and
# above, UINT32_MAX), the submix case included. The three callers are copied
# onto a stand-in source, with the lock, the peeks, every memset and memcpy,
# the callbacks and the volume recorded. Builds the moved file on its own: no
# display, so it runs on every platform, and links libm except on MSVC. The
# functions are hidden in libobs, so this characterizes that C source; the test
# calls only them. test_source_volume_mix, test_source_audio_ranges,
# test_audio_capture_mix_race and test_group_hide_transition_audio cover the
# callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the thirteen hidden functions of obs-source-audio-render.c and
# includes only their header, so it links the archive alone: its obs-source
# member needs nothing from libobs or the other crates. Linking the archive
# directly avoids exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  add_executable(test_source_audio_render_steps_rust test_source_audio_render_steps.c)
  target_include_directories(
    test_source_audio_render_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_source_audio_render_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_source_audio_render_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_source_audio_render_steps_rust)
  add_test(test_source_audio_render_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_source_audio_render_steps_rust)
endif()
