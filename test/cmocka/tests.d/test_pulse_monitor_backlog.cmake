# libobs PulseAudio monitoring: drop the audio queued during a stall
# (obsproject/obs-studio#4531, #3525, #10548). Plain C, no server needed.
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_PULSEAUDIO)
  find_package(PulseAudio REQUIRED)

  add_executable(
    test_pulse_monitor_backlog
    test_pulse_monitor_backlog.c
    "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring/pulse/pulseaudio-stream-flags.c"
  )
  target_include_directories(
    test_pulse_monitor_backlog
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs/audio-monitoring/pulse"
  )
  target_link_libraries(test_pulse_monitor_backlog PRIVATE PulseAudio::PulseAudio ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pulse_monitor_backlog)
  add_test(test_pulse_monitor_backlog ${CMAKE_CURRENT_BINARY_DIR}/test_pulse_monitor_backlog)
endif()
