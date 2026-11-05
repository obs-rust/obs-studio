# Moving scene items into or out of groups keeps activate/show counts balanced
# (obsproject/obs-studio#9961). Hiding a group is applied by the audio thread,
# which needs core video, so it runs (under xvfb-run) on Linux only.
# test_group_active_refs is defined in build/test-graph.json.
