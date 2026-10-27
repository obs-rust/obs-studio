# A streaming PipeWire camera renegotiated for its own format when the
# Properties window opens or reloads (obsproject/obs-studio#10399; libcamera
# cameras then drop the stream): the camera portal against a fake portal on a
# private D-Bus bus and a camera node on a private PipeWire daemon that
# test_pipewire_cursor_hdr.py starts. Needs an X display (xvfb-run) for
# obs_startup(), dbus-daemon and pipewire; skips (77, or cmocka skips) without
# them.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(PipeWire 0.3.60 QUIET)
endif()
if(OS_LINUX AND TARGET OBS::pipewire AND PipeWire_FOUND)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_camera_renegotiate_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_camera_renegotiate_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_camera_renegotiate
    test_pipewire_camera_renegotiate.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/camera-portal.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/portal.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c.h"
  )
  target_include_directories(
    test_pipewire_camera_renegotiate
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_camera_renegotiate_drm_includes}
  )
  target_link_libraries(
    test_pipewire_camera_renegotiate
    PRIVATE
      OBS::libobs
      OBS::glad
      PipeWire::PipeWire
      gio::gio
      PkgConfig::_camera_renegotiate_test_glib
      X11::X11
      ${CMOCKA_LIBRARIES}
  )
  # A build tree has no libobs-opengl on the loader path, so load it by path.
  target_compile_definitions(
    test_pipewire_camera_renegotiate
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  add_dependencies(test_pipewire_camera_renegotiate libobs-opengl)
  add_dependencies(cmocka-tests test_pipewire_camera_renegotiate)
  add_test(
    NAME test_pipewire_camera_renegotiate
    COMMAND
      uv run --script "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_cursor_hdr.py"
      $<TARGET_FILE:test_pipewire_camera_renegotiate>
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_pipewire_camera_renegotiate PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 60)
endif()
