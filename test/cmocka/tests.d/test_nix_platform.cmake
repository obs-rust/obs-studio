# The public nix platform API is selected by the Linux/BSD source group.
if(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  add_executable(test_nix_platform test_nix_platform.c)
  target_include_directories(test_nix_platform PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_nix_platform PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  set_target_properties(test_nix_platform PROPERTIES FOLDER tests)
  add_dependencies(cmocka-tests test_nix_platform)
  add_test(NAME test_nix_platform COMMAND test_nix_platform)
endif()
