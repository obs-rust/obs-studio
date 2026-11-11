# Characterizes media-io/video-io.c before its Rust port.
add_executable(test_video_io test_video_io.c)
target_include_directories(test_video_io PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_io PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

# test_video_io_hidden is registered once in build/test-graph.d.
add_dependencies(cmocka-tests test_video_io)
# obs_startup needs an X display; run under xvfb where available so the
# cases can never silently skip (they assert obs_up loudly either way).
find_program(XVFB_RUN xvfb-run)
if(XVFB_RUN)
  add_test(test_video_io ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_video_io)
else()
  add_test(test_video_io ${CMAKE_CURRENT_BINARY_DIR}/test_video_io)
endif()
