# obs_source_enum_active_sources and obs_source_enum_active_tree (#5729):
# NULL source logs one LOG_DEBUG line; a type without enum_active_sources is
# a no-op; the first reports direct active children in plugin order, the
# second walks depth first and reports each edge after the child's subtree,
# descending only into children with the callback. Private sources under a
# started core.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
