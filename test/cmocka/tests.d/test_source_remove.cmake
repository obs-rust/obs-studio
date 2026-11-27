# obs_source_remove (#5803): a NULL source logs the validity check's line
# once; a live source takes one reference, is marked removed, fires the global
# and source signals, leaves its canvas only when it has one and is not
# private, and releases the reference; a removed source, or one whose last
# reference is gone, changes nothing. Real core under xvfb-run. Registered
# once in build/test-graph.json, consumed by CMake and Cargo.
