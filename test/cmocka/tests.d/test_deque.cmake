# Characterization of the header-inline util/deque.h before its Rust
# equivalent in obs-util (#1546). The functions are static inline, so the test
# exercises the C header in both libobs builds; it links libobs for bmem.
# test_deque is defined in build/test-graph.d.
