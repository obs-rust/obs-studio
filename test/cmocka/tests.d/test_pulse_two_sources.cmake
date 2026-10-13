# End to end for two Audio Input Capture sources on one device at a large
# PipeWire graph quantum (the second one mostly silent): the linux-pulseaudio
# capture code against a private PipeWire sound server started by
# test_pulse_monitor_device.sh. libobs's source calls are stubbed (--wrap), so
# no display is needed. Skipped (exit 77) when PipeWire, WirePlumber or
# pipewire-pulse are not installed.
if(TARGET OBS::pulseaudio)
  find_package(PulseAudio REQUIRED)

  set(_pulse_dir "${CMAKE_SOURCE_DIR}/plugins/linux-pulseaudio")
  add_executable(
    test_pulse_two_sources
    test_pulse_two_sources.c
    "${_pulse_dir}/pulse-input.c"
    "${_pulse_dir}/pulse-reconnect.c"
    "${_pulse_dir}/pulse-timing.c"
    "${_pulse_dir}/pulse-wrapper.c"
  )
  target_include_directories(test_pulse_two_sources PRIVATE ${CMOCKA_INCLUDE_DIR} "${_pulse_dir}")
  target_link_libraries(test_pulse_two_sources PRIVATE OBS::libobs PulseAudio::PulseAudio ${CMOCKA_LIBRARIES})
  target_link_options(
    test_pulse_two_sources
    PRIVATE
      "LINKER:--wrap=obs_source_get_name,--wrap=obs_source_output_audio,--wrap=obs_source_set_async_compensation"
      "LINKER:--wrap=obs_source_audio_output_capture_device_changed,--wrap=obs_source_get_weak_source"
      "LINKER:--wrap=obs_weak_source_addref,--wrap=obs_weak_source_release,--wrap=obs_weak_source_get_source"
      "LINKER:--wrap=obs_source_release,--wrap=obs_obj_get_data,--wrap=obs_queue_task"
  )
  add_dependencies(cmocka-tests test_pulse_two_sources)
  add_test(
    NAME test_pulse_two_sources
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pulse_monitor_device.sh" $<TARGET_FILE:test_pulse_two_sources>
  )
  set_tests_properties(test_pulse_two_sources PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
  unset(_pulse_dir)
endif()
