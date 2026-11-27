# A mixer control (VolumeControl) that outlives its source: on quit the
# queued "deactivate" ran after the source was freed and emitted
# OBSBasic::mixerStatusChanged() from the freed UUID (upstream
# obsproject/obs-studio#14262). Builds the real control on Qt's offscreen
# platform against real libobs, with OBSBasic, OBSApp, the meter and the
# slider replaced by volume-control-stubs/; linked with --wrap (GNU ld) to
# own the UUID, so it needs Qt Widgets and a display for obs_startup().
# Target and runtime contract: ../build/test-graph.json.
# Keep discovery optional and honor explicit UI/frontend disablement.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets)
endif()
