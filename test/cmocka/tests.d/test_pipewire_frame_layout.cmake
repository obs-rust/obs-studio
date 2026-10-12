# Where the PipeWire camera source points an async frame's planes in a
# PipeWire buffer (obsproject/obs-studio#13823, NV12 green or crashing). Fake
# spa_buffers only: no PipeWire daemon, portal or display, so it runs in CI.
if(OS_LINUX AND TARGET OBS::pipewire)
  get_target_property(_frame_layout_drm_includes Libdrm::Libdrm INTERFACE_INCLUDE_DIRECTORIES)
  add_executable(
    test_pipewire_frame_layout
    test_pipewire_frame_layout.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/formats.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-layout.c"
  )
  target_include_directories(
    test_pipewire_frame_layout
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire" ${_frame_layout_drm_includes}
  )
  target_link_libraries(test_pipewire_frame_layout PRIVATE OBS::libobs PipeWire::PipeWire ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pipewire_frame_layout)
  add_test(test_pipewire_frame_layout ${CMAKE_CURRENT_BINARY_DIR}/test_pipewire_frame_layout)
endif()
