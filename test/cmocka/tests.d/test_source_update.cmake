# Actual-library characterization of obs_source_update: merge before dispatch,
# deferred video updates run by the video tick, synchronous callback-then-signal
# order and reentrancy. Starts core video, so it runs under xvfb-run on Linux.
# Target and test are declared in ../build/test-graph.json.
