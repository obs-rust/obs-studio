# Hybrid MP4 audio sample entries for more than two channels (OBS forum
# thread 196049: 5.1 recordings silent in VLC 3). Builds obs-outputs'
# mp4-mux.c into the test and muxes stub encoder packets into memory;
# obs_startup needs a display, so it runs (under xvfb-run) on Linux and
# skips without one.
if(TARGET OBS::outputs)
  add_executable(
    test_mp4_mux_multichannel
    test_mp4_mux_multichannel.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/mp4-mux.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-av1.c"
    $<$<BOOL:${ENABLE_HEVC}>:${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-hevc.c>
  )
  target_include_directories(
    test_mp4_mux_multichannel
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs"
  )
  target_link_libraries(test_mp4_mux_multichannel PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_mp4_mux_multichannel)
  add_test(test_mp4_mux_multichannel ${CMAKE_CURRENT_BINARY_DIR}/test_mp4_mux_multichannel)
  set_tests_properties(test_mp4_mux_multichannel PROPERTIES TIMEOUT 60)
endif()
