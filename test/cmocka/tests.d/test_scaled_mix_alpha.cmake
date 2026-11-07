# A rescaled output must show the same picture as the program where the
# canvas has colour but no alpha, as Subtract blending leaves it
# (obsproject/obs-studio#10746). Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
