# Settings > Hotkeys: Alt+C / Alt+O pressed in a key field pressed the dialog's
# "&Cancel" / "&OK" button instead of being recorded (GNOME and KDE themes,
# obsproject/obs-studio#13385). Builds the frontend helper the key field calls
# and drives a dialog with QTest on Qt's offscreen platform, so it needs Qt
# Widgets and Qt Test but no display and no frontend build.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Test)
    add_executable(
      test_hotkey_capture_mnemonic
      test_hotkey_capture_mnemonic.cpp
      "${CMAKE_SOURCE_DIR}/frontend/settings/OBSHotkeyCapture.cpp"
    )
    target_include_directories(
      test_hotkey_capture_mnemonic
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/settings"
    )
    target_link_libraries(test_hotkey_capture_mnemonic PRIVATE Qt6::Widgets Qt6::Test ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_hotkey_capture_mnemonic)
    add_test(test_hotkey_capture_mnemonic ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_capture_mnemonic)
  endif()
endif()
