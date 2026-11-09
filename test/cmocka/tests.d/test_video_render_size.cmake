# Pin the private C render helper's graphics calls without a GPU. This source
# characterization cannot exercise the future Rust swap until it is reachable.
add_executable(test_video_render_size ../cmocka/test_video_render_size.c)
target_link_libraries(test_video_render_size PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka)
if(OS_WINDOWS)
  target_link_libraries(test_video_render_size PRIVATE OBS::w32-pthreads)
endif()
add_test(NAME test_video_render_size COMMAND test_video_render_size)
add_dependencies(cmocka-tests test_video_render_size)
