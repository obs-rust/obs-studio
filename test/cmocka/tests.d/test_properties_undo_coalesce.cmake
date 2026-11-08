# Filters/Properties: changing two fields within 500 ms gave the first undo
# entry the second field's change too, so Undo, Undo, Redo restored the wrong
# settings (obsproject/obs-studio#10628). Builds the real properties view
# (shared/properties-view) and its Qt helpers without the frontend and runs it
# on Qt's offscreen platform, so it needs Qt Widgets and Qt Test but no display.
# Original Qt fixture; target and runtime contract: ../build/test-graph.json.
# Keep discovery optional and honor explicit UI/frontend disablement.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
endif()
