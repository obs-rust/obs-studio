# obs_source_set_texcoords_centered (#5599): stores texcoords_centered alone,
# logs nothing, and the real libobs getter answers what was stored. The setter
# is hidden in libobs, so the test compiles its extracted C file into itself
# (C source only); the scene rendering tests see the swap through hybrid
# libobs. Test-owned struct obs_source, no core. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
