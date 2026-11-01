# write_packet() sends a packet of up to 64 KiB to the obs-ffmpeg-mux helper in
# one os_process_pipe_write() call, header and data together (#3211). Builds
# obs-ffmpeg-mux.c and wraps os_process_pipe_write() to count the calls; no
# display or OBS core is needed.
if(OS_LINUX)
  add_executable(
    test_ffmpeg_mux_write_packet
    test_ffmpeg_mux_write_packet.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-mux.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/build/obs-ffmpeg.mux-host-posix+.h.c"
  )
  target_include_directories(
    test_ffmpeg_mux_write_packet
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_link_libraries(
    test_ffmpeg_mux_write_packet
    PRIVATE OBS::libobs FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  target_link_options(test_ffmpeg_mux_write_packet PRIVATE "LINKER:--wrap=os_process_pipe_write")
  add_dependencies(cmocka-tests test_ffmpeg_mux_write_packet)
  add_test(test_ffmpeg_mux_write_packet ${CMAKE_CURRENT_BINARY_DIR}/test_ffmpeg_mux_write_packet)
  set_tests_properties(test_ffmpeg_mux_write_packet PROPERTIES TIMEOUT 60)
endif()
