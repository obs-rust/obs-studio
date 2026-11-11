add_executable(test_encoder_media_getters test_encoder_media_getters.c)
target_link_libraries(
  test_encoder_media_getters
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash OBS::native-test-cmocka
)
if(OS_WINDOWS)
  target_link_libraries(test_encoder_media_getters PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_encoder_media_getters)
add_test(NAME test_encoder_media_getters COMMAND test_encoder_media_getters)
set_tests_properties(test_encoder_media_getters PROPERTIES TIMEOUT 10)
