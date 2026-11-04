# Characterizes the signals of the source audio settings before they move to
# their own file and are ported (#3619). Core audio needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display. libobs finds its
# effect files relative to the working directory.
# Defined in ../build/test-graph.json.
