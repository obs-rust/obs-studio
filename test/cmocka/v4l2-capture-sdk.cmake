include_guard(DIRECTORY)
set(HAVE_V4L2_CAPTURE_SDK FALSE)
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(PkgConfig QUIET)
  if(PKG_CONFIG_FOUND)
    pkg_check_modules(_test_v4l2_capture_headers QUIET IMPORTED_TARGET libv4l2)
    find_package(FFmpeg QUIET COMPONENTS avcodec avformat avutil)
    if(
      TARGET PkgConfig::_test_v4l2_capture_headers
      AND TARGET FFmpeg::avcodec
      AND TARGET FFmpeg::avformat
      AND TARGET FFmpeg::avutil
    )
      set(HAVE_V4L2_CAPTURE_SDK TRUE)
    endif()
  endif()
endif()
