# Characterization of os_inhibit_sleep_* on Linux and FreeBSD
# (util/platform-nix-inhibit.c) before its port (#1500): the path with no
# session D-Bus, which falls back to xdg-screensaver. A fake xdg-screensaver
# and a missing bus address keep the user's session out of it.
if(OS_LINUX OR OS_FREEBSD)
  add_executable(test_inhibit_sleep test_inhibit_sleep.c)
  target_include_directories(test_inhibit_sleep PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_inhibit_sleep PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_inhibit_sleep)
  add_test(test_inhibit_sleep ${CMAKE_CURRENT_BINARY_DIR}/test_inhibit_sleep)
  set_tests_properties(test_inhibit_sleep PROPERTIES TIMEOUT 60)
endif()
