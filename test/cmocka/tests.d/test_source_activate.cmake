# obs_source_activate and obs_source_deactivate (#5811), through the four
# public counters: activate raises the source's show_refs and walks its active
# tree once adding 1 to each descendant's, then for MAIN_VIEW the same for
# activate_refs; deactivate lowers each counter (show first, activate only for
# MAIN_VIEW) only when the source's is above zero, with a walk subtracting 1
# from each descendant (not floor-checked). A source at zero is not walked.
# Nothing logs. Real core under xvfb-run. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
