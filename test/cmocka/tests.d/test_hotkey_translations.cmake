# Characterization of the hotkey key-name translations in
# libobs/obs-hotkey-translations.c before their port. It links the real
# libobs and goes through the public API only (obs_hotkeys_set_translations_s,
# obs_key_to_str, obs_key_combination_to_str), so it also covers the hybrid
# libobs. Linux and FreeBSD only: there obs_key_to_str shows the stored name
# of every key the translations name.
if(OS_LINUX OR OS_FREEBSD)
  add_executable(test_hotkey_translations test_hotkey_translations.c)
  target_include_directories(test_hotkey_translations PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_hotkey_translations PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

  add_dependencies(cmocka-tests test_hotkey_translations)
  # obs_startup needs an X display on Linux.
  find_program(XVFB_RUN xvfb-run)
  if(XVFB_RUN)
    add_test(test_hotkey_translations ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_translations)
  else()
    add_test(test_hotkey_translations ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_translations)
  endif()
endif()
