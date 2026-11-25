# obs_source_enum_filters (#5605): a NULL source or callback logs the validity
# check's line once (a NULL callback names 'callback_obj.obj', upstream's
# quirk) and returns; otherwise it calls back last to first under
# filter_mutex, reading the count once and each entry when visited; nothing
# is written and no reference is taken. Test-owned struct obs_source values,
# no core. Registered once in build/test-graph.json, consumed by CMake and
# Cargo.
