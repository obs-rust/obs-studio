# A screen capture frozen after its monitor was switched off or unplugged
# (obsproject/obs-studio#12063): the screencast portal source against a fake
# portal on a private D-Bus bus and a producer stream (the monitor) on a
# private PipeWire daemon that test_pipewire_cursor_hdr.sh starts. Needs an X
# display (xvfb-run) for obs_startup(), dbus-daemon and pipewire; skips (77,
# or cmocka skips) without them.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_screencast_resume_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_screencast_resume_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_screencast_resume
    test_pipewire_screencast_resume.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/portal.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/screencast-portal.c"
  )
  target_include_directories(
    test_pipewire_screencast_resume
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_screencast_resume_drm_includes}
  )
  target_link_libraries(
    test_pipewire_screencast_resume
    PRIVATE
      OBS::libobs
      OBS::glad
      PipeWire::PipeWire
      gio::gio
      PkgConfig::_screencast_resume_test_glib
      X11::X11
      ${CMOCKA_LIBRARIES}
  )
  # A build tree has no libobs-opengl on the loader path, so load it by path.
  target_compile_definitions(
    test_pipewire_screencast_resume
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  add_dependencies(test_pipewire_screencast_resume libobs-opengl)
  add_dependencies(cmocka-tests test_pipewire_screencast_resume)
  add_test(
    NAME test_pipewire_screencast_resume
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_cursor_hdr.sh" $<TARGET_FILE:test_pipewire_screencast_resume>
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_pipewire_screencast_resume PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 120)
endif()
