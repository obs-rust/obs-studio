# Settings > Output: the audio track buttons slid away from their "Audio
# Track" label in a tall Settings window (obsproject/obs-studio#13286). Builds
# the real frontend/forms/OBSBasicSettings.ui with stand-ins for its custom
# widgets (settings-ui-stubs/) and lays it out on Qt's offscreen platform:
# needs Qt Widgets only, no display, no frontend build and no libobs.
find_package(Qt6 QUIET COMPONENTS Widgets)
if(TARGET Qt6::Widgets)
  # uic by hand: a second AUTOUIC target in this directory makes CMake give
  # both autogen targets test_output_timer's .ui header.
  qt_wrap_ui(_settings_ui "${CMAKE_SOURCE_DIR}/frontend/forms/OBSBasicSettings.ui")
  add_executable(test_settings_track_rows test_settings_track_rows.cpp ${_settings_ui})
  target_include_directories(
    test_settings_track_rows
    BEFORE
    PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/settings-ui-stubs" "${CMAKE_CURRENT_BINARY_DIR}" ${CMOCKA_INCLUDE_DIR}
  )
  target_link_libraries(test_settings_track_rows PRIVATE Qt6::Widgets ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_settings_track_rows)
  add_test(test_settings_track_rows ${CMAKE_CURRENT_BINARY_DIR}/test_settings_track_rows)
  # Runs in about a second; a stuck Qt start fails the test, not the CI job.
  set_tests_properties(test_settings_track_rows PROPERTIES TIMEOUT 120)
  cmocka_qt_runtime(test_settings_track_rows Qt6::Core)
  unset(_settings_ui)
endif()
