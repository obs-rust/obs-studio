# Unchanged headless cases live in build/test-graph.json (#1390).
# The production PCH needs these SDK headers even without device access.
set(HAVE_FREETYPE_PLATFORM_SDK FALSE)
if((OS_LINUX OR OS_FREEBSD OR OS_OPENBSD))
  find_package(PkgConfig QUIET)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(_test_text_freetype2_platform_headers QUIET IMPORTED_TARGET freetype2 fontconfig)
    if(TARGET PkgConfig::_test_text_freetype2_platform_headers)
      set(HAVE_FREETYPE_PLATFORM_SDK TRUE)
    endif()
  endif()
endif()
