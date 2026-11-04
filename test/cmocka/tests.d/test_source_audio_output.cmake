# Characterizes the path that puts one audio packet in a source's buffer before
# it moves to its own file and is ported (#3628). obs_startup() needs a display
# on Linux, so it runs under xvfb-run and skips without one.
# Defined in ../build/test-graph.json.
