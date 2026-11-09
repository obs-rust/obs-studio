add_executable(test_source_async_decoupled test_source_async_decoupled.c)
target_link_libraries(
  test_source_async_decoupled
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka
)
if(OS_WINDOWS)
  target_link_libraries(test_source_async_decoupled PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_source_async_decoupled)
add_test(NAME test_source_async_decoupled COMMAND test_source_async_decoupled)
set_tests_properties(test_source_async_decoupled PROPERTIES TIMEOUT 30)
