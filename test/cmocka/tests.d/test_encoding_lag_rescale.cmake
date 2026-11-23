# "Skipped frames due to encoding lag" must count encoders that rescale the
# output (upstream obsproject/obs-studio#11078). Needs core video, so it runs
# (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
