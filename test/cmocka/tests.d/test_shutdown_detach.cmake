# Public shutdown entry point with private-header inspection of the table mutex.
add_executable(test_shutdown_detach test_shutdown_detach.c)
target_link_libraries(
  test_shutdown_detach
  PRIVATE OBS::libobs OBS::native-test-cmocka OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)
add_dependencies(cmocka-tests test_shutdown_detach)
add_test(NAME test_shutdown_detach COMMAND test_shutdown_detach)
set_tests_properties(test_shutdown_detach PROPERTIES TIMEOUT 30)
