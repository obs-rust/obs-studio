# Settings > Output: the Replay Buffer memory estimate loses its warning colour
# once the estimate is back within the recommended maximum
# (obsproject/obs-studio#13171). Builds the frontend helper the settings dialog
# calls and runs it on Qt's offscreen platform, so it needs Qt Widgets but no
# display and no frontend build.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets)
  if(TARGET Qt6::Widgets)
    add_executable(
      test_replay_buffer_estimate
      test_replay_buffer_estimate.cpp
      "${CMAKE_SOURCE_DIR}/frontend/utility/replay-buffer-estimate.cpp"
    )
    target_include_directories(
      test_replay_buffer_estimate
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
    )
    target_link_libraries(test_replay_buffer_estimate PRIVATE Qt6::Widgets ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_replay_buffer_estimate)
    add_test(test_replay_buffer_estimate ${CMAKE_CURRENT_BINARY_DIR}/test_replay_buffer_estimate)
  endif()
endif()
