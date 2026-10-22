# End to end for the samples PulseAudio audio monitoring plays: libobs
# monitoring into null sinks in s16le, s32le, u8 and float32le at source
# volumes 0.5, 1 and 2 (saturation, #1709), recorded from each sink's monitor
# with parec, against a private PipeWire sound server started by
# test_pulse_monitor_device.sh. Skipped (exit 77) when PipeWire, WirePlumber,
# pipewire-pulse or parec are not installed.
if(OS_LINUX AND ENABLE_PULSEAUDIO)
  add_executable(test_pulse_monitor_samples test_pulse_monitor_samples.c)
  target_include_directories(test_pulse_monitor_samples PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_pulse_monitor_samples PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pulse_monitor_samples)
  add_test(
    NAME test_pulse_monitor_samples
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pulse_monitor_device.sh" $<TARGET_FILE:test_pulse_monitor_samples>
  )
  set_tests_properties(test_pulse_monitor_samples PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()
