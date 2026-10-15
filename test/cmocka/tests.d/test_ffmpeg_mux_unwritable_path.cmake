# Start Recording on a path that cannot be written failed, and the next Start
# Recording crashed OBS (obsproject/obs-studio#11940,
# obsproject/obs-websocket#1216): a failed ffmpeg_muxer start released the
# output's settings twice. Builds obs-ffmpeg-mux.c and runs the real output
# against core video, so it runs under xvfb-run on Linux and skips without a
# display. obs_data_release() is wrapped to count the releases.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(
    test_ffmpeg_mux_unwritable_path
    test_ffmpeg_mux_unwritable_path.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/obs-ffmpeg-mux.c"
  )
  target_include_directories(
    test_ffmpeg_mux_unwritable_path
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_compile_definitions(
    test_ffmpeg_mux_unwritable_path
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_link_libraries(
    test_ffmpeg_mux_unwritable_path
    PRIVATE OBS::libobs FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  target_link_options(test_ffmpeg_mux_unwritable_path PRIVATE "LINKER:--wrap=obs_data_release")
  add_dependencies(test_ffmpeg_mux_unwritable_path libobs-opengl)
  add_dependencies(cmocka-tests test_ffmpeg_mux_unwritable_path)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_ffmpeg_mux_unwritable_path
    COMMAND test_ffmpeg_mux_unwritable_path
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_ffmpeg_mux_unwritable_path PROPERTIES TIMEOUT 120)
endif()
