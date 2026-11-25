# obs_source_add_active_child and obs_source_remove_active_child (#5622):
# NULL parent or child logs one LOG_DEBUG line; add refuses parent == child
# with a warning, and a parent anywhere in the child's full tree (as a child
# or as an edge's parent) after one walk with the parent as its stop source;
# otherwise the child gets show_refs(parent) activations, the first
# activate_refs(parent) MAIN_VIEW and the rest AUX_VIEW, and remove mirrors
# them with deactivations, with no walk. Private sources under a started core.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
