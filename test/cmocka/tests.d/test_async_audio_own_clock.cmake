# An async source whose audio runs on a clock of its own, hours from its video
# (NDI Test Patterns through DistroAV), must be heard on the program (#3747).
# Mixing needs core video, so it runs (under xvfb-run) on Linux and skips
# without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
