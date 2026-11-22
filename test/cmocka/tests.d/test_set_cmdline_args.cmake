# Characterization of obs_set_cmdline_args before its Rust swap (#5468): the
# copy is one allocation (pointer table, then the strings back to back), a
# first call with argc 0 stores an empty table and a later call still copies,
# and obs_shutdown() frees the copy. The shutdown case needs a display on
# Linux. Registered once in build/test-graph.json.
