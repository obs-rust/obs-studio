# Characterize the public getter through the selected real libobs on every host.
add_executable(test_source_audio_mixers_getter test_source_audio_mixers_getter.c)
target_include_directories(test_source_audio_mixers_getter PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_source_audio_mixers_getter
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES}
)
if(OS_WINDOWS)
  target_link_libraries(test_source_audio_mixers_getter PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_source_audio_mixers_getter)
if(XVFB_RUN)
  add_test(
    NAME test_source_audio_mixers_getter
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_source_audio_mixers_getter>
  )
else()
  add_test(NAME test_source_audio_mixers_getter COMMAND test_source_audio_mixers_getter)
endif()
set_tests_properties(test_source_audio_mixers_getter PROPERTIES TIMEOUT 30)
