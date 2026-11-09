# Public-API characterization against whichever libobs the harness selects.
add_executable(test_hotkey_unregister test_hotkey_unregister.c)
target_link_libraries(test_hotkey_unregister PRIVATE OBS::libobs OBS::native-test-cmocka)
add_dependencies(cmocka-tests test_hotkey_unregister)
add_test(NAME test_hotkey_unregister COMMAND test_hotkey_unregister)
set_tests_properties(test_hotkey_unregister PROPERTIES TIMEOUT 30)
