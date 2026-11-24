# obs_source_set_async_unbuffered / obs_source_async_compensation (#5595):
# NULL logs the caller's name (and the getter answers false), the setter
# stores async_unbuffered alone, and the getter answers async_compensation
# and writes nothing. Test-owned struct obs_source, no core. Registered once
# in build/test-graph.json, consumed by CMake and Cargo.
