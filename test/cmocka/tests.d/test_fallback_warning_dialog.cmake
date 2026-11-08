# Fallback warning window (obs-rust/obs-studio#2218): the countdown, both
# buttons, Escape and the window close each end the dialog exactly once, with
# the right end value of the C ABI. Builds the real
# OBSFallbackWarning (its own unity group, frontend/build/) without the
# frontend and runs it on Qt's offscreen platform, so it needs Qt Widgets and
# Qt Test but no display.
# Original Qt fixture; target and runtime contract: ../build/test-graph.json.
# Keep discovery optional and honor explicit UI/frontend disablement.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
endif()
