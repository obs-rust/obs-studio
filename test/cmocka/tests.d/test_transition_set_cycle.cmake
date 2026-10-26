# obs_transition_set() must refuse a source that contains the transition, as
# obs_transition_load() does; a walk of such a source's tree hangs. Core video
# is needed, so it runs (under xvfb-run) on Linux only.
# Defined in ../build/test-graph.json.
