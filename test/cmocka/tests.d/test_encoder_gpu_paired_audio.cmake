# The paired audio encoder of a texture encoder starts from the start time that
# the GPU encode thread sets (#3571). Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
# Defined in ../build/test-graph.json.
