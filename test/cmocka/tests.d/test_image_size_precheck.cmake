# An image larger than the graphics device's texture size limit must fail
# before it is decoded, not after (obsproject/obs-studio#3427). Needs core
# video, so it runs (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
