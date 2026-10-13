# A ScreenCast portal reply handled while the capture is destroyed
# (obsproject/obs-studio#13809). Runs the screencast portal code against a fake
# portal on a private D-Bus bus that the test starts itself, so it needs an X
# display (xvfb-run) and dbus-daemon, but no session bus, PipeWire or portal.
# Without them it exits with 77 and ctest reports it as skipped.
if(TARGET OBS::pipewire)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_portal_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_pipewire_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_screencast_portal_teardown
    test_screencast_portal_teardown.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/portal.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/screencast-portal.c"
  )
  target_include_directories(
    test_screencast_portal_teardown
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_pipewire_drm_includes}
  )
  target_link_libraries(
    test_screencast_portal_teardown
    PRIVATE OBS::libobs OBS::glad PipeWire::PipeWire gio::gio PkgConfig::_portal_test_glib X11::X11 ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_screencast_portal_teardown)
  add_test(test_screencast_portal_teardown ${CMAKE_CURRENT_BINARY_DIR}/test_screencast_portal_teardown)
  set_tests_properties(test_screencast_portal_teardown PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()
