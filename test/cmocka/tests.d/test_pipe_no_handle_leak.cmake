# A child started through util/pipe gets only its standard streams, not the
# parent's sockets or files (obsproject/obs-studio#4596).
if(TARGET OBS::libobs)
  add_executable(test_pipe_no_handle_leak test_pipe_no_handle_leak.c)
  target_include_directories(test_pipe_no_handle_leak PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_pipe_no_handle_leak
    PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} $<$<PLATFORM_ID:Windows>:ws2_32>
  )
  add_dependencies(cmocka-tests test_pipe_no_handle_leak)
  add_test(NAME test_pipe_no_handle_leak COMMAND test_pipe_no_handle_leak)
  set_tests_properties(test_pipe_no_handle_leak PROPERTIES TIMEOUT 30)
endif()
