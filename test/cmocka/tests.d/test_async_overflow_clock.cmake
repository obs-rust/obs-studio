# Exercise the actual async cache without starting the graphics thread (#3368).
if(OS_LINUX)
  add_executable(test_async_overflow_clock test_async_overflow_clock.c)
  target_include_directories(test_async_overflow_clock PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_async_overflow_clock PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_async_overflow_clock)
  add_test(NAME test_async_overflow_clock COMMAND test_async_overflow_clock)
endif()
