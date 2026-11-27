# obs_source_restore_filters (#5801): an invalid source or a NULL array logs
# the validity check's line once and changes nothing; otherwise the current
# filters are taken off the list, each array item reuses the first current
# filter of its name (never an unnamed one) or is loaded with filter_mutex
# free, each restored filter targets the next (the last the source), every
# parent is set once every target is, and the old filters are released once
# each, in old order, after the new list is in place. No signals. Real core
# under xvfb-run. Registered once in build/test-graph.json, consumed by CMake
# and Cargo.
