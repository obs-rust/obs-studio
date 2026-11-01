# video_scaler_scale() writes straight into a frame laid out like its internal
# buffer and copies into any other layout; both give the same bytes (#3192).
# Plain libobs, no display or OBS core.
if(OS_LINUX)
  add_executable(test_video_scaler_direct test_video_scaler_direct.c)
  target_include_directories(test_video_scaler_direct PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_video_scaler_direct PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_video_scaler_direct)
  add_test(test_video_scaler_direct ${CMAKE_CURRENT_BINARY_DIR}/test_video_scaler_direct)
  set_tests_properties(test_video_scaler_direct PROPERTIES TIMEOUT 120)
endif()
