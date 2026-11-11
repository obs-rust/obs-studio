# Unchanged pipe-based udev loop cases live in build/test-graph.d (#1390).
set(HAVE_V4L2_UDEV_SDK FALSE)
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(Libudev QUIET)
  if(TARGET Libudev::Libudev)
    set(HAVE_V4L2_UDEV_SDK TRUE)
  endif()
endif()
