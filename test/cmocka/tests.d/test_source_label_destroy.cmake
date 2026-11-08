# The Sources dock's name label (OBSSourceLabel) deleted on the main thread
# while its source is destroyed on libobs's destruction thread: a crash on
# quit in signal_handler_disconnect(). Builds the real label on Qt's offscreen
# platform against a real libobs signal handler; linked with --wrap (GNU ld)
# to play the source and count the handler's references, so it needs Qt
# Widgets but no display, no obs_startup() and no frontend build.
# The original narrow PCH stand-in is now tracked in source-label-stubs/.
# Original Qt fixture; target and runtime contract: ../build/test-graph.json.
# Keep discovery optional and honor explicit UI/frontend disablement.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets)
endif()
