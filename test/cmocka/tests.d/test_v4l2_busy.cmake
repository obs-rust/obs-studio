# Unchanged headless cases live in build/test-graph.json (#1390).
# The production PCH needs these SDK headers even without device access.
set(HAVE_V4L2_BUSY_SDK FALSE)
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(PkgConfig QUIET)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(_test_v4l2_busy_headers QUIET IMPORTED_TARGET libv4l2 libavcodec libavformat libavutil)
    if(TARGET PkgConfig::_test_v4l2_busy_headers)
      set(HAVE_V4L2_BUSY_SDK TRUE)
    endif()
  endif()
endif()
