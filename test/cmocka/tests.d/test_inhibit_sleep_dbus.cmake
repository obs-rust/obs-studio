# Sleep inhibition on Linux and FreeBSD (os_inhibit_sleep_*) through the
# org.freedesktop.portal.Inhibit portal and the org.freedesktop.ScreenSaver
# fallback. Both are faked on a private bus (GTestDBus starts its own
# dbus-daemon; with none the test exits 77, which ctest reports as Skipped and
# Tier 2 refuses, rust/tools/linux-validate/check_skips.py). No display is needed.
if(OS_LINUX OR OS_FREEBSD)
  find_package(Gio QUIET)
  find_package(PkgConfig QUIET)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(INHIBIT_TEST_GLIB QUIET IMPORTED_TARGET glib-2.0 gobject-2.0)
  endif()
  if(TARGET gio::gio AND TARGET PkgConfig::INHIBIT_TEST_GLIB)
    add_executable(test_inhibit_sleep_dbus test_inhibit_sleep_dbus.c)
    target_include_directories(test_inhibit_sleep_dbus PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(
      test_inhibit_sleep_dbus
      PRIVATE OBS::libobs gio::gio PkgConfig::INHIBIT_TEST_GLIB ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_inhibit_sleep_dbus)
    add_test(test_inhibit_sleep_dbus ${CMAKE_CURRENT_BINARY_DIR}/test_inhibit_sleep_dbus)
    set_tests_properties(test_inhibit_sleep_dbus PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
  endif()
endif()
