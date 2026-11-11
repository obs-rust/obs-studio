# Properties: a refresh (obs_source_update_properties) while a field's file,
# font, color or list-entry dialog was open deleted the field's widgets under
# the dialog; picking a value then wrote to freed memory
# (obsproject/obs-studio#10423). Builds the real properties view
# (shared/properties-view) and its Qt helpers without the frontend and runs it
# on Qt's offscreen platform, so it needs Qt Widgets and Qt Test but no display.
# Built with ASan so a freed widget fails the test.
# Original Qt fixture; target and runtime contract: ../build/test-graph.d.
# Keep discovery optional and honor explicit UI/frontend disablement.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
endif()
