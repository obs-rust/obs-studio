# Real public timer wrappers with a fake backend; no GPU.
add_executable(test_timer_dispatch test_timer_dispatch.c)
target_include_directories(test_timer_dispatch PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_timer_dispatch PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_timer_dispatch PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_timer_dispatch)
add_test(NAME test_timer_dispatch COMMAND test_timer_dispatch)
