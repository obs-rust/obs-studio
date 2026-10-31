# The WASAPI monitor's speaker layout for the device's channel mask and
# count (#2559). Shared code, so it runs on every platform.
# Defined in ../build/test-graph.json.

# The same cases against the Rust shim, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). audio_monitor_speaker_layout
# is hidden in the shared library, so the test takes it from the production
# staticlib at executable link time and compiles none of the C. The
# obs-audio-monitoring member needs nothing from libobs, so it links the
# archive alone. Linux only: the shim has no host code, so one host shows the
# swap.
if(TARGET libobs_rust-static)
  if(OS_LINUX)
    add_executable(test_monitor_speaker_layout_rust test_monitor_speaker_layout.c)
    target_include_directories(
      test_monitor_speaker_layout_rust
      PRIVATE "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring" ${CMOCKA_INCLUDE_DIR}
    )
    target_link_libraries(test_monitor_speaker_layout_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES})
    obs_test_sanitize(test_monitor_speaker_layout_rust "address,undefined")
    add_dependencies(cmocka-tests test_monitor_speaker_layout_rust)
    add_test(test_monitor_speaker_layout_rust ${CMAKE_CURRENT_BINARY_DIR}/test_monitor_speaker_layout_rust)
  endif()
endif()
