# A saved PipeWire camera source at startup (obsproject/obs-studio#13971): the
# camera portal against a fake portal on a private D-Bus bus and a camera node
# on a private PipeWire daemon that test_pipewire_cursor_hdr.sh starts. Needs
# an X display (xvfb-run) for obs_startup(), dbus-daemon and pipewire; skips
# (77, or cmocka skips) without them. Linked with --wrap to hold the camera
# portal while the registry delivers its globals.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(PipeWire 0.3.60 QUIET)
endif()
if(OS_LINUX AND TARGET OBS::pipewire AND PipeWire_FOUND)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_camera_registry_race_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_camera_registry_race_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_camera_registry_race
    test_pipewire_camera_registry_race.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/camera-portal.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/portal.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c"
  )
  target_include_directories(
    test_pipewire_camera_registry_race
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_camera_registry_race_drm_includes}
  )
  target_link_libraries(
    test_pipewire_camera_registry_race
    PRIVATE
      OBS::libobs
      OBS::glad
      PipeWire::PipeWire
      gio::gio
      PkgConfig::_camera_registry_race_test_glib
      X11::X11
      ${CMOCKA_LIBRARIES}
  )
  target_link_options(
    test_pipewire_camera_registry_race
    PRIVATE
      "LINKER:--wrap=obs_pipewire_connect_fd"
      "LINKER:--wrap=obs_pipewire_get_registry"
      "LINKER:--wrap=obs_pipewire_connect_stream"
  )
  add_dependencies(cmocka-tests test_pipewire_camera_registry_race)
  add_test(
    NAME test_pipewire_camera_registry_race
    COMMAND
      sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_cursor_hdr.sh" $<TARGET_FILE:test_pipewire_camera_registry_race>
  )
  set_tests_properties(test_pipewire_camera_registry_race PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()
