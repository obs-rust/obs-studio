if(OS_LINUX)
  # read_distribution_info() is static inline in obs-nix-os-release.h and
  # calls the hidden os-release rules of obs-nix-log-rules.c, so the test
  # builds that C file into itself.
  add_executable(test_distribution_info test_distribution_info.c "${CMAKE_SOURCE_DIR}/libobs/obs-nix-log-rules.c.h")
  target_include_directories(test_distribution_info PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_distribution_info PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_distribution_info)
  add_test(test_distribution_info ${CMAKE_CURRENT_BINARY_DIR}/test_distribution_info)

  if(TARGET OBS::glad)
    add_executable(test_egl_init_error test_egl_init_error.c)
    target_include_directories(test_egl_init_error PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs-opengl")
    target_link_libraries(test_egl_init_error PRIVATE OBS::libobs OBS::glad ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_egl_init_error)
    add_test(test_egl_init_error ${CMAKE_CURRENT_BINARY_DIR}/test_egl_init_error)
  endif()
endif()
