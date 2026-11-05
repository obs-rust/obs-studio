# A scene must not be moved into a group the scene itself contains
# (obsproject/obs-studio#10466). Each case forks a child with core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
# test_group_recursion is defined in build/test-graph.json.
