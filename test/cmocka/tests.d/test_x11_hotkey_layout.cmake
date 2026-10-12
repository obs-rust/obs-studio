# X11 hotkeys with a layout that has "[" on a second key
# (obsproject/obs-studio#12941). Loads the reporter's core keymap rows into
# the X server and presses keys with XTEST. Needs an X display (xvfb-run);
# the cases skip without one or without XTEST.
if(OS_LINUX)
  find_package(X11 QUIET)
  if(X11_FOUND AND X11_Xtst_FOUND)
    add_executable(test_x11_hotkey_layout test_x11_hotkey_layout.c)
    target_include_directories(test_x11_hotkey_layout PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(test_x11_hotkey_layout PRIVATE OBS::libobs X11::X11 X11::Xtst ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_x11_hotkey_layout)
    add_test(test_x11_hotkey_layout ${CMAKE_CURRENT_BINARY_DIR}/test_x11_hotkey_layout)
    set_tests_properties(test_x11_hotkey_layout PROPERTIES TIMEOUT 60)
  endif()
endif()
