# linux-capture: a failed X request must not leak its xcb error reply
# (obs-rust/obs-studio#2053). Builds the linux-capture unity file directly, as
# test_xcomposite_window_events does. Skips without a display.
if(OS_LINUX AND TARGET linux-capture AND TARGET OBS::libobs-opengl)
  find_package(X11 REQUIRED)
  find_package(X11_XCB REQUIRED)
  find_package(
    XCB
    REQUIRED XCB XFIXES RANDR SHM XINERAMA COMPOSITE
  )
endif()
# Registration lives in the shared graph. Declare the owning target fact on
# every host: graph requirements are evaluated eagerly.
set(HAVE_LINUX_CAPTURE FALSE)
if(TARGET linux-capture)
  set(HAVE_LINUX_CAPTURE TRUE)
endif()
