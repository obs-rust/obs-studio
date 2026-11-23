# linux-v4l2: a camera another process holds open announces itself to the
# frontend (popup) once, Force Camera closes the holder, and the camera starts
# when the holder lets go (obs-rust/obs-studio#799). Real v4l2-input.c on a
# real libobs source, fake libv4l2, forked helper processes as holders.
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(Libv4l2 REQUIRED)
endif()
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
