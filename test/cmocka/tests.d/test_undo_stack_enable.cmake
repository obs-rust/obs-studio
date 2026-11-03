# Edit > Undo stayed enabled on an empty undo stack after any disable/enable
# span (#2222). Builds the real frontend/utility/undo_stack.cpp.hpp with
# stand-ins for the OBSBasic.ui header and the frontend's precompiled header
# (undo-stack-stubs/) on Qt's offscreen platform: needs Qt Widgets only, no
# display, no frontend build and no libobs.
# Linux only, like test_properties_undo_coalesce: on Windows the offscreen
# platform plugin is not deployed and the test hung to its timeout.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets)
endif()
# Target and test are declared in ../build/test-graph.json.
