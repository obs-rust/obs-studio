# Exit in the tray menu did nothing when OBS started minimized to the tray on
# Wayland (obsproject/obs-studio#13494): the never-shown main window had no
# platform window, so QWidget::close() sent it no close event. Builds the
# frontend helper OBSBasic::close() uses (frontend/utility/close-top-level.hpp)
# and drives a top-level widget in that state on Qt's offscreen platform:
# needs Qt Widgets only, no display and no frontend build.
# test_top_level_windows (was test_close_top_level, #5478) is defined in
# build/test-graph.json. It also holds the quit, floating dock window, moved
# dock and redock cases of the main window.
