# Registration lives in build/test-graph.json; no duplicate executable.
set(HAVE_PIPEWIRE_HEADLESS_SDK FALSE)
set(HAVE_PIPEWIRE_LOOP_TEST_SDK FALSE)
if(TARGET OBS::pipewire)
  find_package(PipeWire QUIET)
  find_package(PkgConfig QUIET)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(_pipewire_headless_drm QUIET IMPORTED_TARGET libdrm)
    pkg_check_modules(_pipewire_headless_gio QUIET IMPORTED_TARGET gio-2.0)
    pkg_check_modules(_pipewire_headless_glib QUIET IMPORTED_TARGET glib-2.0 gobject-2.0)
    if(
      PipeWire_FOUND
      AND TARGET PipeWire::PipeWire
      AND TARGET PkgConfig::_pipewire_headless_drm
      AND TARGET PkgConfig::_pipewire_headless_gio
      AND TARGET PkgConfig::_pipewire_headless_glib
    )
      if(NOT TARGET Libdrm::Libdrm)
        add_library(Libdrm::Libdrm ALIAS PkgConfig::_pipewire_headless_drm)
      endif()
      if(NOT TARGET gio::gio)
        add_library(gio::gio ALIAS PkgConfig::_pipewire_headless_gio)
      endif()
      if(NOT TARGET OBS::native-test-glib)
        add_library(OBS::native-test-glib ALIAS PkgConfig::_pipewire_headless_glib)
      endif()
      set(HAVE_PIPEWIRE_HEADLESS_SDK TRUE)
      if(PipeWire_VERSION VERSION_GREATER_EQUAL 0.3.60)
        set(HAVE_PIPEWIRE_LOOP_TEST_SDK TRUE)
      endif()
    endif()
  endif()
endif()
