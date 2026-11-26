# obs_source_filter_set_order (#5775): a NULL source or filter logs the
# validity check's line once and does nothing; an unlisted filter moves
# nothing; up and down move to the nearest filter of the same async kind, top
# and bottom to the ends; after a move every filter is retargeted (the next
# one, the last the source) and "reorder_filters" is signalled once on the
# source, after filter_mutex is released. Test-owned struct obs_source
# values, no core. Registered once in build/test-graph.json, consumed by
# CMake and Cargo.
