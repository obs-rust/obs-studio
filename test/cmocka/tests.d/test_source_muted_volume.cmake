# source_muted() and get_source_volume() (#5853) through the capture path
# and the program mix. Mixing needs core video, so it runs (under xvfb-run)
# on Linux and skips without a display. Canonical target and test:
# test/cmocka/build/test-graph.json.
