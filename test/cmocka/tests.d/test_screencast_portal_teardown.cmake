# A ScreenCast portal reply handled while the capture is destroyed
# (obsproject/obs-studio#13809). Runs the screencast portal code against a fake
# portal on a private D-Bus bus that the test starts itself, so it needs an X
# display (xvfb-run) and dbus-daemon, but no session bus, PipeWire or portal.
# Without them it exits with 77 and ctest reports it as skipped.
set(HAVE_OBS_PIPEWIRE FALSE)
if(TARGET OBS::pipewire)
  set(HAVE_OBS_PIPEWIRE TRUE)
  find_package(X11 REQUIRED)
  find_package(PkgConfig REQUIRED)
  pkg_check_modules(_portal_test_glib REQUIRED IMPORTED_TARGET glib-2.0 gobject-2.0)
endif()
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
