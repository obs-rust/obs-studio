# Registration lives in build/test-graph.json; no duplicate executable.
set(HAVE_LINUX_CAPTURE FALSE)
if(TARGET linux-capture)
  set(HAVE_LINUX_CAPTURE TRUE)
endif()
if(OS_LINUX AND TARGET linux-capture)
  find_package(X11 REQUIRED)
  find_package(X11_XCB REQUIRED)
  find_package(
    XCB
    REQUIRED XCB XFIXES RANDR SHM XINERAMA COMPOSITE
  )
endif()
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
