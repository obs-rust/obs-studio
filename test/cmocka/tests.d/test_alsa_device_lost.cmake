# linux-alsa: an Audio Capture Device source stayed silent after its USB device
# was unplugged and plugged in again, while its capture thread spun on the dead
# handle (#2924). The capture code runs on ALSA's null PCM, with snd_pcm_open
# and snd_pcm_readi wrapped to act like a USB device that goes away and comes
# back (--wrap), and libobs's source calls stubbed, so no display and no sound
# card are needed.
if(TARGET OBS::alsa)
  find_package(ALSA REQUIRED)

  add_executable(test_alsa_device_lost test_alsa_device_lost.c "${CMAKE_SOURCE_DIR}/plugins/linux-alsa/alsa-input.c.h")
  target_include_directories(test_alsa_device_lost PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_alsa_device_lost PRIVATE OBS::libobs ALSA::ALSA ${CMOCKA_LIBRARIES})
  target_link_options(
    test_alsa_device_lost
    PRIVATE
      "LINKER:--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio,--wrap=snd_pcm_open,--wrap=snd_pcm_readi"
  )
  add_dependencies(cmocka-tests test_alsa_device_lost)
  add_test(test_alsa_device_lost ${CMAKE_CURRENT_BINARY_DIR}/test_alsa_device_lost)
  set_tests_properties(test_alsa_device_lost PROPERTIES TIMEOUT 120)
endif()
