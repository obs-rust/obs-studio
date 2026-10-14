# linux-alsa: destroying an ALSA source whose device is missing left its
# reopen thread running on freed memory, and OBS crashed a second later
# (residue of obsproject/obs-studio#9322, obsproject/obs-studio#13480). The
# capture code runs on an ALSA device name that does not exist; libobs's source
# calls are stubbed (--wrap), so no display and no sound card are needed.
if(TARGET OBS::alsa)
  find_package(ALSA REQUIRED)

  add_executable(
    test_alsa_reopen_destroy
    test_alsa_reopen_destroy.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-alsa/alsa-input.c"
  )
  target_include_directories(test_alsa_reopen_destroy PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_alsa_reopen_destroy PRIVATE OBS::libobs ALSA::ALSA ${CMOCKA_LIBRARIES})
  target_link_options(
    test_alsa_reopen_destroy
    PRIVATE "LINKER:--wrap=obs_source_set_async_compensation,--wrap=obs_source_output_audio"
  )
  add_dependencies(cmocka-tests test_alsa_reopen_destroy)
  add_test(test_alsa_reopen_destroy ${CMAKE_CURRENT_BINARY_DIR}/test_alsa_reopen_destroy)
  set_tests_properties(test_alsa_reopen_destroy PROPERTIES TIMEOUT 60)
endif()
