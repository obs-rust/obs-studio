# A PipeWire camera sending NV12 (obsproject/obs-studio#13823) or only I420 or
# YV12 (obsproject/obs-studio#13744): the camera source's stream code
# (pipewire.c) against a producer stream on a private PipeWire daemon that
# test_pipewire_cursor_hdr.sh starts, with the output read back. Needs an X display (xvfb-run) for obs_startup() and pipewire; skips
# (77, or cmocka skips) without them. The no-display half is
# test_pipewire_frame_layout.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_camera_nv12_test_glib REQUIRED IMPORTED_TARGET glib-2.0)
  get_target_property(_camera_nv12_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_camera_nv12
    test_pipewire_camera_nv12.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c"
  )
  target_include_directories(
    test_pipewire_camera_nv12
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_camera_nv12_drm_includes}
  )
  target_link_libraries(
    test_pipewire_camera_nv12
    PRIVATE
      OBS::libobs
      OBS::glad
      PipeWire::PipeWire
      gio::gio
      PkgConfig::_camera_nv12_test_glib
      X11::X11
      ${CMOCKA_LIBRARIES}
  )
  # A build tree has no libobs-opengl on the loader path, so load it by path.
  target_compile_definitions(test_pipewire_camera_nv12 PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  add_dependencies(test_pipewire_camera_nv12 libobs-opengl)
  add_dependencies(cmocka-tests test_pipewire_camera_nv12)
  add_test(
    NAME test_pipewire_camera_nv12
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_cursor_hdr.sh" $<TARGET_FILE:test_pipewire_camera_nv12>
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_pipewire_camera_nv12 PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 90)
endif()
