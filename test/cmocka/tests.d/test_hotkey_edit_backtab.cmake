# Settings > Hotkeys: Shift+Tab out of a key field bound "Shift" to that
# action before the focus moved back (obsproject/obs-studio#12475). Builds the
# frontend helper the key field calls and drives a key field made like
# OBSHotkeyEdit on Qt's offscreen platform, so it needs Qt Widgets and Qt Test
# but no display and no frontend build.
find_package(Qt6 QUIET COMPONENTS Widgets Test)
if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
  add_executable(
    test_hotkey_edit_backtab
    test_hotkey_edit_backtab.cpp
    "${CMAKE_SOURCE_DIR}/frontend/settings/OBSHotkeyCapture.cpp"
  )
  target_include_directories(
    test_hotkey_edit_backtab
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/settings"
  )
  target_link_libraries(test_hotkey_edit_backtab PRIVATE Qt6::Widgets Qt6::Test ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_hotkey_edit_backtab)
  add_test(test_hotkey_edit_backtab ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_edit_backtab)
  cmocka_qt_runtime(test_hotkey_edit_backtab Qt6::Core)
endif()
