# Wayland hotkey key state and the GlobalShortcuts portal client
# (obsproject/obs-studio#10701, #13169, #10538). Both run the real libobs
# hotkey thread on an isolated core; no Wayland display is needed.
if(OS_LINUX OR OS_FREEBSD)
  add_executable(
    test_wayland_hotkeys
    test_wayland_hotkeys.c
    "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-bindings.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-translations.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-press.c.h"
    "${CMAKE_SOURCE_DIR}/libobs/obs-nix-hotkey-state.c.h"
  )
  target_compile_definitions(test_wayland_hotkeys PRIVATE obs=hotkey_test_obs)
  target_include_directories(test_wayland_hotkeys PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
  target_link_libraries(test_wayland_hotkeys PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_wayland_hotkeys)
  add_test(test_wayland_hotkeys ${CMAKE_CURRENT_BINARY_DIR}/test_wayland_hotkeys)

  # A fake org.freedesktop.portal.GlobalShortcuts on a private bus
  # (GTestDBus starts its own dbus-daemon; skipped when there is none).
  find_package(Gio QUIET)
  find_package(PkgConfig QUIET)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(PORTAL_TEST_GLIB QUIET IMPORTED_TARGET glib-2.0 gobject-2.0)
  endif()
  if(TARGET gio::gio AND TARGET PkgConfig::PORTAL_TEST_GLIB)
    add_executable(
      test_portal_shortcuts
      test_portal_shortcuts.c
      "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-bindings.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-translations.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-hotkey-press.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-hotkey-state.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-portal-shortcut-ids.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-portal-shortcut-sets.c.h"
      "${CMAKE_SOURCE_DIR}/libobs/obs-nix-portal-shortcuts.c.h"
    )
    target_compile_definitions(test_portal_shortcuts PRIVATE obs=hotkey_test_obs)
    target_include_directories(test_portal_shortcuts PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/libobs")
    target_link_libraries(
      test_portal_shortcuts
      PRIVATE OBS::libobs OBS::caption gio::gio PkgConfig::PORTAL_TEST_GLIB ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_portal_shortcuts)
    add_test(test_portal_shortcuts ${CMAKE_CURRENT_BINARY_DIR}/test_portal_shortcuts)
  endif()
endif()
