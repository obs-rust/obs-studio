# obs_load_sources, the scene-collection load, on a live core (#5571): pass 1
# creates every entry before pass 2 loads any, then each source's load and the
# callback in array order, a transition entry's saved state, and that the call
# leaves no reference of its own behind. Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
