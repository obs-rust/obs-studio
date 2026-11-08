# A source set to Darken blending must leave the background where it is
# transparent, not draw black there (obsproject/obs-studio#6591). Needs core
# video, so it runs (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
