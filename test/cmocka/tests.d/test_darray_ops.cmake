# Characterization of the util/darray.h operations the output interleaver uses
# (insert, erase, erase range, pop front, push back new), before the Rust
# equivalent in obs-util gains them (#1594). The functions are static inline,
# so the test exercises the C header in both libobs builds; it links libobs for
# bmem.
# test_darray_ops is defined in build/test-graph.d.
