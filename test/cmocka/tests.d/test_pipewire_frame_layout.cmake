# Unchanged plane-layout cases live in build/test-graph.json (#1390).
# The production PCH also includes DRM, GIO/GLib and GLAD headers, even though
# these cases need no GL implementation, display, device or PipeWire daemon.
set(HAVE_PIPEWIRE_FRAME_LAYOUT_SDK FALSE)
if(OS_LINUX AND TARGET OBS::pipewire)
  find_package(PipeWire QUIET)
  find_package(PkgConfig QUIET)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(_frame_layout_test_headers QUIET IMPORTED_TARGET libdrm gio-2.0)
    if(TARGET PipeWire::PipeWire AND TARGET PkgConfig::_frame_layout_test_headers)
      set(HAVE_PIPEWIRE_FRAME_LAYOUT_SDK TRUE)
    endif()
  endif()
endif()
