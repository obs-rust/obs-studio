# obs_save_sources_filtered and obs_save_sources, the scene-collection save,
# on a live core (#5564): which sources they keep, in which order, what the
# filter callback sees, and that the save keeps each source's refcount. Needs
# core video, so it runs (under xvfb-run) on Linux and skips without a
# display. Registered once in build/test-graph.json, consumed by CMake and
# Cargo.
