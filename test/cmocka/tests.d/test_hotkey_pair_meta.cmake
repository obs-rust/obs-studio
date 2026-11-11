# Public API characterization runs against both actual libobs implementations.
add_executable(test_hotkey_pair_meta test_hotkey_pair_meta.c)
target_link_libraries(test_hotkey_pair_meta PRIVATE OBS::libobs OBS::native-test-cmocka)
add_dependencies(cmocka-tests test_hotkey_pair_meta)
if(XVFB_RUN)
  add_test(
    NAME test_hotkey_pair_meta
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_hotkey_pair_meta>
  )
else()
  add_test(NAME test_hotkey_pair_meta COMMAND test_hotkey_pair_meta)
endif()
set_tests_properties(test_hotkey_pair_meta PROPERTIES TIMEOUT 30)
