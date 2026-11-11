# bnum_allocs() must be stable once obs_startup() returns: background threads
# may not allocate lazily afterwards (obs-rust/obs-studio#663). obs_startup()
# needs a display on Linux, so the case skips without one.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
