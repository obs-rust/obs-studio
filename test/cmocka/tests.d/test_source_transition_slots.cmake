# Characterization of the child slots of a transition in
# libobs/obs-source-transition.c before they move to their own file and are
# ported to rust/obs-source: obs_transition_clear(),
# obs_transition_enum_sources() and the swap (obs_transition_swap_begin(),
# copy_source_state(), obs_transition_swap_end()). Pins what each slot holds,
# which transition holds a reference on it, and the clock and render targets
# the swap hands over, through the real libobs;
# test_transition_swap, test_transition_swap_active and
# test_transition_swap_cycle cover the swap's running state and cycles.
# Transitions need core video, so it builds on Linux only and runs under
# xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
