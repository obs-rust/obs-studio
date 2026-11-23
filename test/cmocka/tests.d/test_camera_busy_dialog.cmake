# Camera-busy warning window (obs-rust/obs-studio#3724): Close it, Ignore,
# Always allow, Escape and the window close each answer exactly once, nothing
# answers without a click, and the window is a non-modal child of the main
# window that never stays on top. Builds the real OBSCameraBusy (its own unity
# group, frontend/build/) without the frontend and runs it on Qt's offscreen
# platform, so it needs Qt Widgets and Qt Test but no display.
# Original Qt fixture; target and runtime contract: ../build/test-graph.d.
# Keep discovery optional and honor explicit UI/frontend disablement.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
endif()
