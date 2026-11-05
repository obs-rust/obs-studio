# Settings > Output: the audio track buttons slid away from their "Audio
# Track" label in a tall Settings window (obsproject/obs-studio#13286). Builds
# the real frontend/forms/OBSBasicSettings.ui with stand-ins for its custom
# widgets (settings-ui-stubs/) and lays it out on Qt's offscreen platform:
# needs Qt Widgets only, no display, no frontend build and no libobs.
find_package(Qt6 QUIET COMPONENTS Widgets)
# Target, UI generation and runtime contract: ../build/test-graph.json (canonical).
