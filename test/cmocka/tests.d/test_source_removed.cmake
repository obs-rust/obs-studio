# obs_source_removed (#5585): NULL is true and logs the caller's name, the
# answer is the removed flag alone, nothing is written, and the flag is read
# atomically while another thread changes it. Test-owned struct obs_source,
# no core. Registered once in build/test-graph.json, consumed by CMake and
# Cargo.
