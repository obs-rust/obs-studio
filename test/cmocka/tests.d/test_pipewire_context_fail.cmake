# PipeWire connection setup failures must not crash OBS: a missing client
# configuration (the obs-rust AppImage shipped without client.conf) made
# pw_context_new() return NULL, which obs_pipewire_connect_fd() then passed on.
# The failure cases need no daemon and always run; the control case uses a
# private PipeWire daemon that test_pipewire_context_fail.sh starts when
# pipewire is installed. Needs no display.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_context_fail_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_context_fail_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_context_fail
    test_pipewire_context_fail.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/pipewire.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c"
  )
  target_include_directories(
    test_pipewire_context_fail
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_context_fail_drm_includes}
  )
  target_link_libraries(
    test_pipewire_context_fail
    PRIVATE OBS::libobs OBS::glad PipeWire::PipeWire PkgConfig::_context_fail_test_glib ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_pipewire_context_fail)
  add_test(
    NAME test_pipewire_context_fail
    COMMAND sh "${CMAKE_CURRENT_SOURCE_DIR}/test_pipewire_context_fail.sh" $<TARGET_FILE:test_pipewire_context_fail>
  )
  set_tests_properties(test_pipewire_context_fail PROPERTIES TIMEOUT 60)
endif()
