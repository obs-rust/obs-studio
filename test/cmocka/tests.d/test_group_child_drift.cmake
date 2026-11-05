# Moving one child of a group must not drag its siblings across the canvas
# (obsproject/obs-studio#9298). Groups resize on video ticks, so this needs
# core video and runs (under xvfb-run) on Linux only.
# test_group_child_drift is defined in build/test-graph.json.
