# Characterization of the hotkey press state machine in libobs before its
# port. It links the real libobs on X11 and goes through the public API only
# (obs_hotkey_load_bindings, obs_hotkey_inject_event, callback rerouting,
# background presses), holding keys with XTEST so the hotkey thread polls a
# real key state. It runs under xvfb-run on its own X server; without a
# display or XTEST the cases skip.
if(OS_LINUX)
  find_package(X11 QUIET)
  if(X11_FOUND AND X11_Xtst_FOUND)
    add_executable(test_hotkey_press_state test_hotkey_press_state.c)
    target_include_directories(test_hotkey_press_state PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(test_hotkey_press_state PRIVATE OBS::libobs X11::X11 X11::Xtst ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_hotkey_press_state)
    # Its own X server, not the display the other tests share (XVFB_RUN,
    # xvfb-run-or-display.sh): it holds keys, and held modifiers would reach
    # the other tests on that display.
    if(OBS_XVFB_RUN_REAL)
      add_test(
        NAME test_hotkey_press_state
        COMMAND
          ${OBS_XVFB_RUN_REAL} -a -s "-screen 0 640x480x24 -noreset" ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_press_state
      )
    else()
      add_test(test_hotkey_press_state ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_press_state)
    endif()
    set_tests_properties(test_hotkey_press_state PROPERTIES TIMEOUT 120)
  endif()
endif()
