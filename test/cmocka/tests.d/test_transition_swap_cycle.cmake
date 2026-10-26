# obs_transition_swap_begin() must not store a source that contains the
# destination transition; a walk of such a source's tree hangs. Core video
# is needed, so it runs (under xvfb-run) on Linux only.
# Defined in ../build/test-graph.json.
