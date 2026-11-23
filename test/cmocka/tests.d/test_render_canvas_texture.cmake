# The four canvas texture draw entry points (obs_render_main_texture and
# friends) drawn through a real graphics device into a render target and read
# back (#5530). Needs core video, so it runs (under xvfb-run) on Linux and
# skips without a display. Registered once in build/test-graph.json, consumed
# by CMake and Cargo.
