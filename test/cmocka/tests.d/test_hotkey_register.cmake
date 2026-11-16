# Characterize the public single-hotkey registration entry points against the
# real libobs. The Tier 2 runner selects hybrid or C-reference via the loader.
add_executable(test_hotkey_register test_hotkey_register.c)
target_include_directories(test_hotkey_register PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_hotkey_register PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_hotkey_register)
set(_hotkey_register_display)
if(XVFB_RUN)
  set(_hotkey_register_display "${XVFB_RUN}" -a)
endif()
add_test(NAME test_hotkey_register COMMAND ${_hotkey_register_display} $<TARGET_FILE:test_hotkey_register>)
set_tests_properties(test_hotkey_register PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 30)
unset(_hotkey_register_display)
