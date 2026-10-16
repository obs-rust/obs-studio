# linux-alsa: a device that offers only S24_3LE and S16_LE (common on USB
# audio interfaces) was captured at 16 bits. The capture code runs on ALSA's
# null PCM with the device's formats and samples stubbed (--wrap), so no
# display and no sound card are needed.
if(TARGET OBS::alsa)
  find_package(ALSA REQUIRED)

  add_executable(
    test_alsa_capture_format
    test_alsa_capture_format.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-alsa/alsa-input.c"
  )
  target_include_directories(test_alsa_capture_format PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_alsa_capture_format PRIVATE OBS::libobs ALSA::ALSA ${CMOCKA_LIBRARIES})
  target_link_options(
    test_alsa_capture_format
    PRIVATE
      "LINKER:--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio,--wrap=snd_pcm_hw_params_test_format,--wrap=snd_pcm_readi"
  )
  add_dependencies(cmocka-tests test_alsa_capture_format)
  add_test(test_alsa_capture_format ${CMAKE_CURRENT_BINARY_DIR}/test_alsa_capture_format)
  set_tests_properties(test_alsa_capture_format PROPERTIES TIMEOUT 60)
endif()
