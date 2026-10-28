# The X11 key table names Volume Up and Down the right way round
# (obs-rust/obs-studio#2635). Starts libobs on X11. Needs an X display
# (xvfb-run); the case skips without one.
if(OS_LINUX)
  find_package(X11 QUIET)
  if(X11_FOUND)
    add_executable(test_x11_volume_keys test_x11_volume_keys.c)
    target_include_directories(test_x11_volume_keys PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(test_x11_volume_keys PRIVATE OBS::libobs X11::X11 ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_x11_volume_keys)
    add_test(test_x11_volume_keys ${CMAKE_CURRENT_BINARY_DIR}/test_x11_volume_keys)
    set_tests_properties(test_x11_volume_keys PROPERTIES TIMEOUT 60)
  endif()
endif()
