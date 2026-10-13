# Screen capture frame pacing (upstream obsproject/obs-studio#13292). The
# pacing decisions are compiled into the test directly; no PipeWire daemon,
# portal or graphics context is needed.
if(TARGET OBS::pipewire)
  add_executable(
    test_pipewire_frame_pacing
    test_pipewire_frame_pacing.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire/frame-pacing.c"
  )
  target_include_directories(
    test_pipewire_frame_pacing
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/linux-pipewire"
  )
  target_link_libraries(test_pipewire_frame_pacing PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pipewire_frame_pacing)
  add_test(test_pipewire_frame_pacing ${CMAKE_CURRENT_BINARY_DIR}/test_pipewire_frame_pacing)
endif()
