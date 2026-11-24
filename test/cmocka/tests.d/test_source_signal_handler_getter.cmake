# obs_source_get_signal_handler (#5591): NULL answers NULL and logs the
# caller's name, otherwise the answer is context.signals itself (borrowed,
# NULL when the source has none) and nothing is written. Test-owned struct
# obs_source, no core. Registered once in build/test-graph.json, consumed by
# CMake and Cargo.
