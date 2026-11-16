# Characterize both source audio-pause subscriptions before their Rust port.
add_executable(test_source_audio_pause_callbacks test_source_audio_pause_callbacks.c)
target_include_directories(test_source_audio_pause_callbacks PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_source_audio_pause_callbacks
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES} "$<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>"
)
add_dependencies(cmocka-tests test_source_audio_pause_callbacks)
add_test(NAME test_source_audio_pause_callbacks COMMAND test_source_audio_pause_callbacks)
set_tests_properties(test_source_audio_pause_callbacks PROPERTIES TIMEOUT 30)
