# The video thread must be running for obs_wait_for_destroy_queue() to sync with it.
if(OS_LINUX)
  add_executable(test_destroy_queue_video_sync test_destroy_queue_video_sync.c)
  target_include_directories(
    test_destroy_queue_video_sync
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/deps/libcaption"
  )
  target_link_libraries(test_destroy_queue_video_sync PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  target_compile_definitions(
    test_destroy_queue_video_sync
    PRIVATE TEST_LIBOBS_DATA_PATH="${CMAKE_SOURCE_DIR}/libobs/data/" GRAPHICS_MODULE_PATH="$<TARGET_FILE:libobs-opengl>"
  )
  add_dependencies(cmocka-tests test_destroy_queue_video_sync)
  add_test(NAME test_destroy_queue_video_sync COMMAND test_destroy_queue_video_sync)
  set_tests_properties(test_destroy_queue_video_sync PROPERTIES TIMEOUT 30)
endif()
