# C-only regression for the pipe creation/exec race (#266). Compile the real
# implementation into the executable so its syscalls can be wrapped even when
# libobs is shared or ENABLE_RUST_LIBOBS is ON. No desktop session is needed.
if(CMAKE_SYSTEM_NAME STREQUAL "Linux")
  add_executable(test_pipe_cloexec test_pipe_cloexec.c "${CMAKE_SOURCE_DIR}/libobs/util/pipe-posix.c")
  target_link_libraries(test_pipe_cloexec PRIVATE OBS::libobs)
  target_link_options(test_pipe_cloexec PRIVATE "LINKER:--wrap=pipe" "LINKER:--wrap=pipe2")
  add_dependencies(cmocka-tests test_pipe_cloexec)
  add_test(test_pipe_cloexec ${CMAKE_CURRENT_BINARY_DIR}/test_pipe_cloexec)
  set_tests_properties(test_pipe_cloexec PROPERTIES TIMEOUT 30)
endif()
