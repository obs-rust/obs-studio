# obs_source_copy_filters and obs_source_copy_single_filter (#5792): a NULL
# argument logs the validity check's line once and copies nothing; the
# filters are copied in the source's order in front of the destination's own,
# named with the next free number, with their settings and enabled state
# (always private, as libobs makes every filter), and one filter only when
# compatible. Real core under xvfb-run. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
