# Characterization of libobs/callback/signal.c.h before its port (#1441).
# ASan's allocator moves every reallocated block, so the cases that grow a
# callback array while it is signalling do not depend on the platform
# allocator. libobs itself is not instrumented.
# Defined in ../build/test-graph.d.
