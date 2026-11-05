# Adding a source to an empty group with a bounding box must not give it a
# NaN transform (obsproject/obs-studio#4245). Renders scenes, so this needs
# core video and runs (under xvfb-run) on Linux only; the cases skip without
# a display.
# test_group_empty_bounds is defined in build/test-graph.json.
