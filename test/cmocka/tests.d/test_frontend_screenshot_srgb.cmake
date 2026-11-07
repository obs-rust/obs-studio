# The frontend's "Screenshot (Source)" must encode a high-bit-depth source
# (P010, I010, I210: linear float in libobs) to sRGB, so the PNG shows the
# colours of the program output instead of a darker picture. Builds the
# screenshot render helper (frontend/utility/screenshot-render.cpp) against
# real libobs and the built OpenGL module, with Qt Widgets for the headers
# display-helpers.hpp includes, so it runs where the frontend is off (Tier 2).
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
if(OS_LINUX AND TARGET OBS::libobs-opengl AND NOT TARGET Qt6::Widgets)
  find_package(Qt6 QUIET COMPONENTS Widgets)
endif()
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
