# Actual-library C characterization; no native test graph changes.
add_executable(test_output_reconnect_callback test_output_reconnect_callback.c)
target_include_directories(test_output_reconnect_callback PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_output_reconnect_callback
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES}
)
if(OS_WINDOWS)
  target_link_libraries(test_output_reconnect_callback PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_output_reconnect_callback)
add_test(NAME test_output_reconnect_callback COMMAND test_output_reconnect_callback)
set_tests_properties(test_output_reconnect_callback PROPERTIES TIMEOUT 30)
