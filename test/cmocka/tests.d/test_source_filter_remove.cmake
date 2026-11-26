# obs_source_filter_remove (#5795): a NULL source or filter logs the validity
# check's line once and does nothing; an unlisted filter changes nothing; a
# listed one is erased (the entry before it takes its target), then, with
# filter_mutex free, the global and source signals, the debug line, the
# type's filter_remove callback, the parent and target cleared and one
# release; destroying a source removes its filters from the first. Real core
# under xvfb-run. Registered once in build/test-graph.json, consumed by CMake
# and Cargo.
