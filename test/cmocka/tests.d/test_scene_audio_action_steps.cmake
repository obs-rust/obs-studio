# Characterization of the scene item visibility audio ramp in
# libobs/obs-scene-audio-actions.c before its port: the 0/1 gain buffer, the
# actions consumed and the hides counted, sample by sample, with a fake scene
# item. Builds the moved file on its own: no display and no libobs core, so
# it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them, so the port can run
# the same file against the Rust staticlib (as test_scene_bounds_place_rust
# does). test_scene_audio_actions covers the callers through the real libobs.
add_executable(
  test_scene_audio_action_steps
  test_scene_audio_action_steps.c
  "${CMAKE_SOURCE_DIR}/libobs/obs-scene-audio-actions.c.h"
)
target_include_directories(
  test_scene_audio_action_steps
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(test_scene_audio_action_steps PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
obs_test_sanitize(test_scene_audio_action_steps "address,undefined")

add_dependencies(cmocka-tests test_scene_audio_action_steps)
add_test(test_scene_audio_action_steps ${CMAKE_CURRENT_BINARY_DIR}/test_scene_audio_action_steps)
