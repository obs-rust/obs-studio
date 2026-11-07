# A libobs-opengl dynamic texture's map, write and unmap hands the exact bytes
# to the next draw, without re-specifying the texture on every unmap (#3191).
# Runs the real GL module under xvfb-run on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
