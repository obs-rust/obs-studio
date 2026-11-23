# An output that was started but never received a packet must finish
# stopping, and destroying it must return (#3739). Needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
