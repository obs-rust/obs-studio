# Actual-library test, separate from the unchanged source-only characterization.
add_executable(test_encoder_group_lifecycle_api test_encoder_group_lifecycle_api.c)
target_link_libraries(
  test_encoder_group_lifecycle_api
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka
)
if(OS_WINDOWS)
  target_link_libraries(test_encoder_group_lifecycle_api PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_encoder_group_lifecycle_api)
add_test(NAME test_encoder_group_lifecycle_api COMMAND test_encoder_group_lifecycle_api)
set_tests_properties(test_encoder_group_lifecycle_api PROPERTIES RUN_SERIAL TRUE)
