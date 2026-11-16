# Public depth-state dispatch with recording callbacks; GPU-free on both libraries.
add_executable(test_depth_dispatch test_depth_dispatch.c)
target_include_directories(test_depth_dispatch PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_depth_dispatch PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_depth_dispatch PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_depth_dispatch)
add_test(NAME test_depth_dispatch COMMAND test_depth_dispatch)
