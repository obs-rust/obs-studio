# C-source characterization of the private helper, following stop_requested.
# Recording unmap callbacks pin identity/order and per-slot resets without a GPU.
# This does not exercise a future Rust swap: retain live-caller Tier 2 coverage.
add_executable(test_video_unmap_last_surface test_video_unmap_last_surface.c)
target_include_directories(test_video_unmap_last_surface PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_video_unmap_last_surface PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} OBS::caption Uthash::Uthash)
if(TARGET OBS::w32-pthreads)
  target_link_libraries(test_video_unmap_last_surface PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_video_unmap_last_surface)
add_test(NAME test_video_unmap_last_surface COMMAND test_video_unmap_last_surface)
set_tests_properties(test_video_unmap_last_surface PROPERTIES TIMEOUT 15)
