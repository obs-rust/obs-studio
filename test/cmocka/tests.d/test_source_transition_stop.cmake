# Characterization of the transition clock read and stop helpers of
# libobs/obs-source-transition.c before they move to their own file and are
# ported to rust/obs-source-seam: obs_transition_get_time() in auto and manual
# mode, obs_transition_force_stop()'s callback and signals (public, private,
# without a callback), and the slots, signals and active children after an
# auto transition stops, rendered directly and on an output channel, through
# the real libobs. Transitions need core video, so it builds on Linux only and
# runs under xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
