# obs_add_data_path and obs_remove_data_path, the process-global list of extra
# data directories (#5355): copied caller bytes, duplicates, first equal
# removal and the order it leaves, NULL/empty, and that the list outlives
# startup and shutdown. The lifecycle case needs a display on Linux (run under
# xvfb-run). Registered once in build/test-graph.json, consumed by CMake and
# Cargo.
