# Characterization of the GlobalShortcuts portal client's combination set
# logic in libobs/obs-nix-portal-shortcut-sets.c before its port. Builds the
# set logic on its own: no GIO, no D-Bus and no libobs core. The functions
# are hidden in libobs, so this characterizes that C source; the test calls
# only them, so the port can run the same file against the Rust staticlib
# (as test_scene_bounds_place_rust does).
if(OS_LINUX OR OS_FREEBSD)
  add_executable(
    test_portal_shortcut_sets
    test_portal_shortcut_sets.c
    "${CMAKE_SOURCE_DIR}/libobs/obs-nix-portal-shortcut-sets.c.h"
  )
  target_include_directories(
    test_portal_shortcut_sets
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
  )
  target_link_libraries(test_portal_shortcut_sets PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

  add_dependencies(cmocka-tests test_portal_shortcut_sets)
  add_test(test_portal_shortcut_sets ${CMAKE_CURRENT_BINARY_DIR}/test_portal_shortcut_sets)
endif()
