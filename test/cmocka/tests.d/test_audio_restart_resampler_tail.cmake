# A Media Source started again must not first play the end of its previous
# playback held in the resampler (obsproject/obs-studio#7063). Starts libobs
# core audio; like the other core tests it runs on Linux only.
if(OS_LINUX)
  add_executable(test_audio_restart_resampler_tail test_audio_restart_resampler_tail.c)
  target_include_directories(test_audio_restart_resampler_tail PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_audio_restart_resampler_tail PRIVATE OBS::libobs OBS::caption m ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_audio_restart_resampler_tail)
  add_test(test_audio_restart_resampler_tail ${CMAKE_CURRENT_BINARY_DIR}/test_audio_restart_resampler_tail)
endif()
