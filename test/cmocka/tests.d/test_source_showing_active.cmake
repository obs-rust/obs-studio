# obs_source_active and obs_source_showing (#5400): NULL is false and logs the
# caller's name, each predicate is its own counter being nonzero, nothing is
# written, and the counters are read atomically while another thread changes
# them. Test-owned struct obs_source, no core. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
