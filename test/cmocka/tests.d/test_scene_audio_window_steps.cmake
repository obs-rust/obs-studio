# Characterization of the audio window decisions of scene_audio_render() in
# libobs/obs-scene.c, moved to libobs/obs-scene-audio-window.c before their
# port: the source each item is read from for every combination of
# visibility and running transition, the tick's earliest timestamp over item
# sets (pending items, hidden items, no timestamps), each item's window at
# frame AUDIO_OUTPUT_FRAMES - 1, at it and + 1 for the rates OBS offers, a
# timestamp before the minimum (the uint64_t wrap), the uint64_t and rate
# edges, and every skip rule. Builds the moved file on its own: no display and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them. The
# real-libobs scene audio tests cover the caller.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the hidden functions and includes only their header and
# media-io/audio-io.h (no SIMDe, no obsconfig.h), so it links the archive
# alone: its obs-scene member needs nothing from libobs or the other crates.
# Linking the archive directly avoids exporting private symbols just for a
# test.
if(TARGET libobs_rust-static)
  add_executable(test_scene_audio_window_steps_rust test_scene_audio_window_steps.c)
  target_include_directories(
    test_scene_audio_window_steps_rust
    PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_scene_audio_window_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
  if(NOT MSVC)
    target_link_libraries(test_scene_audio_window_steps_rust PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_scene_audio_window_steps_rust)
  add_test(test_scene_audio_window_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_scene_audio_window_steps_rust)
endif()
