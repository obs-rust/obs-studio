# End to end for an Audio Output Capture set to "Default" when the default
# output changes or is unplugged (obs-rust/obs-studio#2260, upstream
# obsproject/obs-studio#4828, #3211): the real linux-pulseaudio module in the
# real libobs, against a private PulseAudio daemon (test_pulse_pulseaudio.sh)
# and a private pipewire-pulse (test_pulse_monitor_device.sh), with null
# sinks and a pipe source as the microphone. No audio reaches a device.
# Each run skips (exit 77) when its sound server, pactl or pacat is not
# installed. obs_startup needs a display: it runs under XVFB_RUN.
if(OS_LINUX AND TARGET OBS::pulseaudio)
  add_executable(test_pulse_default_follow test_pulse_default_follow.c)
  target_compile_definitions(
    test_pulse_default_follow
    PRIVATE
      PULSE_MODULE_PATH="$<TARGET_FILE:linux-pulseaudio>"
      PULSE_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/linux-pulseaudio/data"
  )
  target_include_directories(test_pulse_default_follow PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_pulse_default_follow PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_pulse_default_follow linux-pulseaudio)
  add_dependencies(cmocka-tests test_pulse_default_follow)

  set(_pulse_display)
  if(XVFB_RUN)
    set(_pulse_display "${XVFB_RUN}" -a)
  endif()
  add_test(
    NAME test_pulse_default_follow
    COMMAND
      ${_pulse_display} sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pulse_pulseaudio.sh"
      $<TARGET_FILE:test_pulse_default_follow>
  )
  add_test(
    NAME test_pulse_default_follow_pipewire
    COMMAND
      ${_pulse_display} sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pulse_monitor_device.sh"
      $<TARGET_FILE:test_pulse_default_follow>
  )
  set_tests_properties(
    test_pulse_default_follow
    test_pulse_default_follow_pipewire
    PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 120
  )
  unset(_pulse_display)
endif()
