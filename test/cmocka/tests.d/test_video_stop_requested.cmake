# C-source characterization of the private stop_requested traversal. The fixture
# substitutes the global handle and stopped query only; real mutex operations
# assert the lock spans every query and is released on return. This pins the C
# source, not a Rust swap. Tier 2 video lifecycle tests cover the real callers.
# Target and test are declared in ../build/test-graph.json.
