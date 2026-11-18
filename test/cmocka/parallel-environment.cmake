# Eight concurrent test processes must share the software-rendering CPU budget.
# Mesa otherwise starts a worker pool sized to the machine for each renderer.
if(OS_LINUX)
  # Private PulseAudio/PipeWire servers still compete for real-time audio and
  # daemon startup resources (#4944). Apply this after both tests.d and the
  # native graph register their tests, for the shared OFF/ON Tier 2 tree.
  # Keep unrelated tests parallel and preserve any existing resource locks.
  foreach(
    _audio_daemon_test
    IN
    ITEMS
      test_pulse_default_follow
      test_pulse_default_follow_pipewire
      test_pulse_monitor_device
      test_pulse_monitor_samples
      test_pulse_stream_restart
      test_pulse_two_sources
      test_pipewire_camera_control_refresh
      test_pipewire_camera_format_dedup
      test_pipewire_camera_nv12
      test_pipewire_camera_registry_race
      test_pipewire_camera_renegotiate
      test_pipewire_camera_resolution_restart
      test_pipewire_camera_unload
      test_pipewire_camera_unplug
      test_pipewire_context_fail
      test_pipewire_cursor_hdr
      test_pipewire_screencast_resume
      test_pipewire_screencast_shm
      test_pipewire_wrapper
      test_v4l2_vmic_libpulse
      test_v4l2_vmic_server
  )
    if(TEST ${_audio_daemon_test})
      set_property(TEST ${_audio_daemon_test} APPEND PROPERTY RESOURCE_LOCK obs_audio_daemon)
    endif()
  endforeach()
  unset(_audio_daemon_test)
  get_property(_cmocka_registered_tests DIRECTORY PROPERTY TESTS)
  if(_cmocka_registered_tests)
    set_property(TEST ${_cmocka_registered_tests} APPEND PROPERTY ENVIRONMENT "LP_NUM_THREADS=1")
  endif()
endif()
