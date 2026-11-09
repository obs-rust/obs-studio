# Actual-library characterization through public encoder creation in both Tier 2 modes.
add_executable(test_encoder_init test_encoder_init.c)
target_link_libraries(test_encoder_init PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka)
if(OS_WINDOWS)
  target_link_libraries(test_encoder_init PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_encoder_init)
add_test(NAME test_encoder_init COMMAND test_encoder_init)
