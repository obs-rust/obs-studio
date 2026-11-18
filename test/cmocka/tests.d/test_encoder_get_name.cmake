# Actual-library characterization of obs_encoder_get_name (#4239) in both Tier 2 modes.
add_executable(test_encoder_get_name test_encoder_get_name.c)
target_link_libraries(test_encoder_get_name PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka)
if(OS_WINDOWS)
  target_link_libraries(test_encoder_get_name PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_encoder_get_name)
add_test(NAME test_encoder_get_name COMMAND test_encoder_get_name)
