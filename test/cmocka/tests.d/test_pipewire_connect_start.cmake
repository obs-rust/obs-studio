# Real connection failure with delayed loop entry (#3594); no daemon needed.
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_connect_start_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
  get_target_property(_connect_start_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_connect_start
    test_pipewire_connect_start.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/reconnect-policy.c.h"
  )
  target_include_directories(
    test_pipewire_connect_start
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_connect_start_drm_includes}
  )
  target_link_libraries(
    test_pipewire_connect_start
    PRIVATE OBS::libobs OBS::glad PipeWire::PipeWire gio::gio PkgConfig::_connect_start_test_glib ${CMOCKA_LIBRARIES}
  )
  target_link_options(
    test_pipewire_connect_start
    PRIVATE
      "LINKER:--wrap=pw_thread_loop_lock"
      "LINKER:--wrap=pw_thread_loop_unlock"
      "LINKER:--wrap=pw_thread_loop_start"
      "LINKER:--wrap=pw_thread_loop_stop"
      "LINKER:--wrap=pw_context_connect_fd"
  )
  add_dependencies(cmocka-tests test_pipewire_connect_start)
  add_test(test_pipewire_connect_start ${CMAKE_CURRENT_BINARY_DIR}/test_pipewire_connect_start)
  set_tests_properties(test_pipewire_connect_start PROPERTIES TIMEOUT 15)
endif()
