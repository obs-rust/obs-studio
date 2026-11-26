# obs_source_get_last_async_ts (#5766): answers async_last_rendered_ts exactly,
# writes nothing and logs nothing. The getter is hidden in libobs, so the test
# compiles its extracted C file into itself (C source only); the source
# profiler reaches the swap through hybrid libobs. Test-owned struct
# obs_source, no core. Registered once in build/test-graph.json, consumed by
# CMake and Cargo.
