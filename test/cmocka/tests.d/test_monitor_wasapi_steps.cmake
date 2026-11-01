# Characterization of the WASAPI monitor's speaker layout and video stall
# tracker, moved from the static inline bodies of monitor-speaker-layout.h and
# monitor-video-sync.h to libobs/audio-monitoring/monitor-speaker-layout.c and
# monitor-video-sync.c: every mask with 0 to 9 channels, 7 channels and counts
# past 8, and the tracker fed a 0 timestamp, the same one, earlier ones and a
# count that wraps. Builds the moved files on their own: no Windows header and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# Target and test are declared in ../build/test-graph.json.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). audio_monitor_speaker_layout
# and audio_monitor_track_video_ts are hidden in the shared library, so the
# test takes them from the production staticlib at executable link time and
# compiles none of the C. The obs-audio-monitoring member needs nothing from
# libobs, so it links the archive alone. Linux only: the shims have no host
# code, so one host shows the swap.
if(TARGET libobs_rust-static)
  if(OS_LINUX)
    add_executable(test_monitor_wasapi_steps_rust test_monitor_wasapi_steps.c)
    target_include_directories(
      test_monitor_wasapi_steps_rust
      PRIVATE "${CMAKE_SOURCE_DIR}/libobs" ${CMOCKA_INCLUDE_DIR}
    )
    target_link_libraries(test_monitor_wasapi_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
    obs_test_sanitize(test_monitor_wasapi_steps_rust "address,undefined")
    add_dependencies(cmocka-tests test_monitor_wasapi_steps_rust)
    add_test(test_monitor_wasapi_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_monitor_wasapi_steps_rust)
  endif()
endif()
