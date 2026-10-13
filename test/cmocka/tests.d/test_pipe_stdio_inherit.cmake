# POSIX pipe creators inherit the standard stream they do not replace.
if(UNIX)
  add_executable(test_pipe_stdio_inherit test_pipe_stdio_inherit.c)
  target_include_directories(test_pipe_stdio_inherit PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_pipe_stdio_inherit PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_pipe_stdio_inherit)
  add_test(test_pipe_stdio_inherit ${CMAKE_CURRENT_BINARY_DIR}/test_pipe_stdio_inherit)
endif()
