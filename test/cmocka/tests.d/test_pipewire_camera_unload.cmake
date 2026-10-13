# The camera portal unloading (OBS quitting) while its PipeWire thread runs:
# the camera portal against a fake portal on a private D-Bus bus and a camera
# node on a private PipeWire daemon that test_pipewire_cursor_hdr.sh starts.
# Needs an X display (xvfb-run) for obs_startup(), dbus-daemon and pipewire;
# skips (77, or cmocka skips) without them. Linked with --wrap to check that
# PipeWire proxies are destroyed on the loop thread or under the loop lock.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(PipeWire 0.3.60 QUIET)
endif()
if(OS_LINUX AND TARGET OBS::pipewire AND PipeWire_FOUND)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_camera_unload_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_camera_unload_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_camera_unload
    test_pipewire_camera_unload.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/camera-portal.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/portal.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c"
  )
  target_include_directories(
    test_pipewire_camera_unload
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_camera_unload_drm_includes}
  )
  target_link_libraries(
    test_pipewire_camera_unload
    PRIVATE
      OBS::libobs
      OBS::glad
      PipeWire::PipeWire
      gio::gio
      PkgConfig::_camera_unload_test_glib
      X11::X11
      ${CMOCKA_LIBRARIES}
  )
  target_link_options(
    test_pipewire_camera_unload
    PRIVATE
      "LINKER:--wrap=pw_thread_loop_new"
      "LINKER:--wrap=pw_thread_loop_lock"
      "LINKER:--wrap=pw_thread_loop_unlock"
      "LINKER:--wrap=pw_thread_loop_stop"
      "LINKER:--wrap=pw_proxy_destroy"
  )
  add_dependencies(cmocka-tests test_pipewire_camera_unload)
  add_test(
    NAME test_pipewire_camera_unload
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_cursor_hdr.sh" $<TARGET_FILE:test_pipewire_camera_unload>
  )
  set_tests_properties(test_pipewire_camera_unload PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()
