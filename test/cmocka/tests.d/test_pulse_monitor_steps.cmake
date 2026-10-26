# Characterization of the PulseAudio monitor's open and write steps of
# libobs/audio-monitoring/pulse/pulseaudio-output.c, moved to
# pulseaudio-monitor-steps.c and ported to rust/obs-audio-monitoring
# (pulse_monitor): do_stream_write()'s grow,
# cork and write rules (the uint32_t tlength * 2 and the truncated new tlength,
# characterized, not endorsed), on_audio_playback()'s skip, byte count and
# volume rules, pulseaudio_sink_info()'s results, audio_monitor_init()'s
# device, feedback, fallback, format, 25 ms buffer and flags, on_sink_event()'s
# filter, audio_monitor_reset()'s context rule and audio_monitor_destroy()'s
# last monitor. Stand-ins put the write loop, the open routes and the reset
# routes together in the C's order. Builds the moved file and
# pulseaudio-stream-flags.c (for pulseaudio_monitor_stream_flags()) into
# itself; no server. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them. test_pulse_monitor_device,
# test_pulse_default_follow, test_pulse_monitor_reset,
# test_pulse_stream_restart, test_pulse_two_sources and
# test_pulse_monitor_samples cover the callers through the real libobs against
# a private sound server.
# Target and test are declared in ../build/test-graph.json.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the twenty hidden functions and pulseaudio_monitor_stream_flags()
# and includes their headers (libpulse's <pulse/def.h> and <pulse/sample.h>,
# headers only), so it links the archive alone: the obs-audio-monitoring
# member needs nothing from libobs. Linking the archive directly avoids
# exporting private symbols just for a test.
if(TARGET libobs_rust-static)
  if((OS_LINUX OR OS_FREEBSD) AND ENABLE_PULSEAUDIO)
    find_package(PulseAudio REQUIRED)
    add_executable(test_pulse_monitor_steps_rust test_pulse_monitor_steps.c)
    target_include_directories(
      test_pulse_monitor_steps_rust
      PRIVATE
        "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring/pulse"
        $<TARGET_PROPERTY:PulseAudio::PulseAudio,INTERFACE_INCLUDE_DIRECTORIES>
        ${CMOCKA_INCLUDE_DIR}
    )
    target_link_libraries(test_pulse_monitor_steps_rust PRIVATE libobs_rust-static ${CMOCKA_LIBRARIES} m)
    add_dependencies(cmocka-tests test_pulse_monitor_steps_rust)
    add_test(test_pulse_monitor_steps_rust ${CMAKE_CURRENT_BINARY_DIR}/test_pulse_monitor_steps_rust)
  endif()
endif()
