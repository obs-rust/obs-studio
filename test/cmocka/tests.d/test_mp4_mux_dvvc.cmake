# Build the MP4 muxer's dvvC box writer from mp4-mux.c and check
# its ISOBMFF framing (size field, "dvvC" type, verbatim payload).
if(TARGET OBS::outputs)
  add_executable(
    test_mp4_mux_dvvc
    test_mp4_mux_dvvc.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/mp4-mux.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-av1.c"
    $<$<BOOL:${ENABLE_HEVC}>:${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-hevc.c>
  )
  target_include_directories(test_mp4_mux_dvvc PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs")
  target_link_libraries(test_mp4_mux_dvvc PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_mp4_mux_dvvc)
  add_test(test_mp4_mux_dvvc ${CMAKE_CURRENT_BINARY_DIR}/test_mp4_mux_dvvc)
endif()
