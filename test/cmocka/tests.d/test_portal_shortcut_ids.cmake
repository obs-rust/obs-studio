# Characterization of the GlobalShortcuts portal shortcut id and trigger codec
# in libobs/obs-nix-portal-shortcut-ids.c before its port (#1517). Builds the
# codec on its own: no GIO, no D-Bus and no libobs core.
if(OS_LINUX OR OS_FREEBSD)
  add_executable(
    test_portal_shortcut_ids
    test_portal_shortcut_ids.c
    "${CMAKE_SOURCE_DIR}/libobs/obs-nix-portal-shortcut-ids.c.h"
  )
  target_include_directories(
    test_portal_shortcut_ids
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
  )
  target_link_libraries(test_portal_shortcut_ids PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

  add_dependencies(cmocka-tests test_portal_shortcut_ids)
  add_test(test_portal_shortcut_ids ${CMAKE_CURRENT_BINARY_DIR}/test_portal_shortcut_ids)
endif()
