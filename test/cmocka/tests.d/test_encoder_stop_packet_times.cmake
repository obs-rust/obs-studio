# Stopping an output must not change an encoder's packet timing array while the
# video thread can still be encoding (obs-rust/obs-studio#3788). Reads the array
# through obs-internal.h on the video thread. Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
