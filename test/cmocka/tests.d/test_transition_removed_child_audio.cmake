# A transition's audio must not come from a removed child after the audio
# stop (#2185). Needs core video, so it runs (under xvfb-run) on Linux and
# skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
