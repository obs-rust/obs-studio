# Actual-library characterization of obs_encoder_set_group (#4645) in both Tier 2 modes.
add_executable(test_encoder_set_group test_encoder_set_group.c)
target_link_libraries(test_encoder_set_group PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka)
if(OS_WINDOWS)
  target_link_libraries(test_encoder_set_group PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_encoder_set_group)
add_test(NAME test_encoder_set_group COMMAND test_encoder_set_group)
