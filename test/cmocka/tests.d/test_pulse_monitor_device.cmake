# End to end for the audio monitoring device on PulseAudio
# (obsproject/obs-studio#10342): libobs monitoring against a private PipeWire
# sound server started by test_pulse_monitor_device.py. Skipped (exit 77)
# when PipeWire, WirePlumber or pipewire-pulse are not installed.
if(OS_LINUX AND ENABLE_PULSEAUDIO)
  find_package(PulseAudio REQUIRED)

  add_executable(test_pulse_monitor_device test_pulse_monitor_device.c)
  target_include_directories(test_pulse_monitor_device PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_pulse_monitor_device PRIVATE OBS::libobs PulseAudio::PulseAudio ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pulse_monitor_device)
  add_test(
    NAME test_pulse_monitor_device
    COMMAND
      uv run --script "${CMAKE_CURRENT_SOURCE_DIR}/test_pulse_monitor_device.py"
      $<TARGET_FILE:test_pulse_monitor_device>
  )
  set_tests_properties(test_pulse_monitor_device PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()
