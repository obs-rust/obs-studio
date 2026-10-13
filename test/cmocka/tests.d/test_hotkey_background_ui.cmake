# Settings -> Hotkeys status banner, row markers and Hyprland config lines
# for Wayland background hotkeys (obs-rust/obs-studio#347). Pure logic
# from the frontend: no Qt, no display, no desktop; the banner texts are
# read from the shipped en-US.ini.
if(OS_LINUX OR OS_FREEBSD)
  find_package(nlohmann_json 3.11 QUIET)
  if(TARGET nlohmann_json::nlohmann_json)
    add_executable(
      test_hotkey_background_ui
      test_hotkey_background_ui.cpp
      "${CMAKE_SOURCE_DIR}/frontend/settings/OBSHotkeyBackground.cpp"
    )
    target_include_directories(
      test_hotkey_background_ui
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/settings"
    )
    target_link_libraries(
      test_hotkey_background_ui
      PRIVATE OBS::libobs nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES}
    )
    target_compile_definitions(
      test_hotkey_background_ui
      PRIVATE HOTKEY_LOCALE_EN_US="${CMAKE_SOURCE_DIR}/frontend/data/locale/en-US.ini"
    )
    add_dependencies(cmocka-tests test_hotkey_background_ui)
    add_test(test_hotkey_background_ui ${CMAKE_CURRENT_BINARY_DIR}/test_hotkey_background_ui)
  endif()
endif()
