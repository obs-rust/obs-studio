# obs_source_enum_full_tree (#5833): NULL source logs one LOG_DEBUG line; a
# root without data or without either enum callback is a no-op; each node
# descends through enum_all_sources, else enum_active_sources, depth first,
# reporting each edge after the child's subtree; a child without data is
# reported, not walked; obs_source_add_active_child's stop source is
# reported, not walked. Private sources under a started core.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
