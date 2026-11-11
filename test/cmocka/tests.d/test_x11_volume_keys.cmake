# The X11 key table names Volume Up and Down the right way round
# (obs-rust/obs-studio#2635). Starts libobs on X11. Needs an X display
# (xvfb-run); the case skips without one.
if(OS_LINUX)
  find_package(X11 QUIET)
endif()
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
