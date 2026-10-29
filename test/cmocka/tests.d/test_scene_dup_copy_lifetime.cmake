# obs_scene_duplicate() must keep each item's copy alive until it returns, so
# a later item with the same source never takes a reference to a copy that an
# "item_add" handler removed. The case forks a child with core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
# libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.json.
