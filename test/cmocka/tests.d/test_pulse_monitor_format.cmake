# libobs PulseAudio monitoring: sample format, channel map, volume, the
# catch-up arithmetic and the feedback-loop device match, before their Rust
# port. Compiles pulseaudio-monitor-format.c into itself (C only); no server
# needed.
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_PULSEAUDIO)
  find_package(PulseAudio REQUIRED)

  add_executable(
    test_pulse_monitor_format
    test_pulse_monitor_format.c
    "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring/pulse/pulseaudio-monitor-format.c.h"
  )
  target_include_directories(
    test_pulse_monitor_format
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring/pulse"
  )
  target_link_libraries(test_pulse_monitor_format PRIVATE OBS::libobs PulseAudio::PulseAudio ${CMOCKA_LIBRARIES})
  obs_test_sanitize(test_pulse_monitor_format "address,undefined")
  add_dependencies(cmocka-tests test_pulse_monitor_format)
  add_test(test_pulse_monitor_format ${CMAKE_CURRENT_BINARY_DIR}/test_pulse_monitor_format)
endif()
