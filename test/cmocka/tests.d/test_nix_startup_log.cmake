# The distribution, desktop and session lines of the Linux startup log
# through the real libobs (obs_startup() runs log_system_info()), so the
# hybrid build checks the Rust log rules (rust/obs-core, nix_system_info)
# through the callers in libobs/obs-nix.c and obs-nix-os-release.h, and
# libobs-c-reference checks the C. The expected distribution comes from
# read_distribution_info() over the same /etc/os-release; that header calls
# the hidden rules of obs-nix-log-rules.c, so the test builds that C file
# into itself as the expectation.
if(OS_LINUX)
  add_executable(test_nix_startup_log test_nix_startup_log.c "${CMAKE_SOURCE_DIR}/libobs/obs-nix-log-rules.c.h")
  target_include_directories(test_nix_startup_log PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_nix_startup_log PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_nix_startup_log)
  add_test(test_nix_startup_log ${CMAKE_CURRENT_BINARY_DIR}/test_nix_startup_log)
  set_tests_properties(test_nix_startup_log PROPERTIES TIMEOUT 120)
endif()
