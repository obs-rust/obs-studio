# This formatter is implemented in obs-nix.c.h only on Linux/FreeBSD.
if(OS_LINUX OR OS_FREEBSD)
  add_executable(test_nix_key_combination test_nix_key_combination.c)
  target_include_directories(test_nix_key_combination PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_nix_key_combination PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_nix_key_combination)
  if(XVFB_RUN)
    add_test(
      NAME test_nix_key_combination
      COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_nix_key_combination>
    )
  else()
    add_test(NAME test_nix_key_combination COMMAND test_nix_key_combination)
  endif()
  set_tests_properties(test_nix_key_combination PROPERTIES TIMEOUT 30)
endif()
