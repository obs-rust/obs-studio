# obs_source_filter_add (#5799): a NULL source or filter logs the validity
# check's line once and does nothing; a listed filter warns with
# filter_mutex held and changes nothing; an incompatible filter is refused
# silently unless the source's type is missing; a dying filter logs the
# 'filter' line and is not added; otherwise the filter goes to the front
# (parent the source, target the old first filter or the source), then, with
# filter_mutex free, the global and source signals, the debug line and the
# type's filter_add callback. Real core under xvfb-run. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
