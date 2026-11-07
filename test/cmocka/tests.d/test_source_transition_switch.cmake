# Characterization of the transition start and set of
# libobs/obs-source-transition.c before they move to their own file and are
# ported to rust/obs-source: obs_transition_start() and obs_transition_set().
# Pins each start's result, slots, callback count and start and stop
# signals, the fixed duration, a re-entering start callback, the set's
# cleared slots and refusal, and which children are active on an output
# channel, through the real libobs. Transitions need core video, so it builds
# on Linux only and runs under xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
