# #3630: the real default-change event task, with deterministic source-info
# errors and a manually dispatched timer. No sound server or display needed.
if(OS_LINUX AND TARGET OBS::pulseaudio)
  find_package(PulseAudio REQUIRED)
  set(_pulse_retry_dir "${CMAKE_SOURCE_DIR}/plugins/linux-pulseaudio")
  add_executable(
    test_pulse_default_retry
    test_pulse_default_retry.c
    "${_pulse_retry_dir}/pulse-reconnect.c.h"
    "${_pulse_retry_dir}/pulse-timing.c.h"
  )
  target_include_directories(
    test_pulse_default_retry
    PRIVATE "${CMAKE_SOURCE_DIR}" "${_pulse_retry_dir}" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_pulse_default_retry PRIVATE OBS::libobs PulseAudio::PulseAudio ${CMOCKA_LIBRARIES})
  target_link_options(
    test_pulse_default_retry
    PRIVATE
      "LINKER:--wrap=obs_weak_source_addref,--wrap=obs_weak_source_release,--wrap=obs_weak_source_get_source"
      "LINKER:--wrap=obs_obj_get_data,--wrap=obs_source_release,--wrap=obs_queue_task"
      "LINKER:--wrap=obs_source_set_async_compensation"
  )
  add_dependencies(cmocka-tests test_pulse_default_retry)
  add_test(NAME test_pulse_default_retry COMMAND test_pulse_default_retry)
  set_tests_properties(test_pulse_default_retry PROPERTIES TIMEOUT 10)
  unset(_pulse_retry_dir)
endif()
