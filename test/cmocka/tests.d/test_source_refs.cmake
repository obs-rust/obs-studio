# obs_source_release, obs_source_get_ref and obs_weak_source_get_source
# (#5818; the internal obs_source_addref is covered by Tier 1 and Tier 3):
# NULL does nothing; get_ref and release move only the control's strong
# count; the release from 0 to -1 destroys the source, then drops its own
# weak reference; get_ref and the weak promotion add 1 to a live source and
# refuse one at -1; with the core shut down, release warns once and touches
# nothing. Real core under xvfb-run. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
