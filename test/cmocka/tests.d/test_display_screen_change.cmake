# A preview display moved to a screen with another pixel ratio (#4010): the
# real OBSQTDisplay on Qt's offscreen platform with two screens (pixel ratio
# 1 and 2) must report DisplayResized when only the screen changes. Needs Qt
# Widgets, no native window, no obs_startup() and no frontend build.
# Original Qt fixture; target and runtime contract: ../build/test-graph.d.
# Keep discovery optional and honor explicit UI/frontend disablement.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets)
endif()
