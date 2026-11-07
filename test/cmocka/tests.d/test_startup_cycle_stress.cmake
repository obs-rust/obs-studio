# Regression test for obs-rust/obs-studio#684: cycling obs_startup/obs_shutdown
# without pause must not crash (hotkey ready event use-after-free on macOS).
# obs_startup() needs a display on Linux, so the case skips without one.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
