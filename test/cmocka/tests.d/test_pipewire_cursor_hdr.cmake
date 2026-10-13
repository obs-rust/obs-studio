# The PipeWire capture cursor in Rec. 2100 (PQ/HLG) output
# (obsproject/obs-studio#13350): the screencast portal source against a fake
# portal on a private D-Bus bus and a producer stream with cursor metadata on
# a private PipeWire daemon that test_pipewire_cursor_hdr.sh starts. Needs an
# X display (xvfb-run) for obs_startup(), dbus-daemon and pipewire; skips
# (77, or cmocka skips) without them.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_cursor_hdr_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_cursor_hdr_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_cursor_hdr
    test_pipewire_cursor_hdr.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/portal.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/screencast-portal.c"
  )
  target_include_directories(
    test_pipewire_cursor_hdr
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_cursor_hdr_drm_includes}
  )
  target_link_libraries(
    test_pipewire_cursor_hdr
    PRIVATE
      OBS::libobs
      OBS::glad
      PipeWire::PipeWire
      gio::gio
      PkgConfig::_cursor_hdr_test_glib
      X11::X11
      ${CMOCKA_LIBRARIES}
  )
  # A build tree has no libobs-opengl on the loader path, so load it by path.
  target_compile_definitions(test_pipewire_cursor_hdr PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  add_dependencies(test_pipewire_cursor_hdr libobs-opengl)
  add_dependencies(cmocka-tests test_pipewire_cursor_hdr)
  add_test(
    NAME test_pipewire_cursor_hdr
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_cursor_hdr.sh" $<TARGET_FILE:test_pipewire_cursor_hdr>
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_pipewire_cursor_hdr PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 90)
endif()
