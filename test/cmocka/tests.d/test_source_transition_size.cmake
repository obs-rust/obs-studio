# Characterization of the transition sizing and tick of
# libobs/obs-source-transition.c before they move to their own file and are
# ported to rust/obs-source: get_cx() and get_cy(), the matrix and size
# recalculations and obs_transition_tick(). Pins the actual size against
# the children, the render size against a set size, the matrices each tick
# rebuilds, and the manual value each tick moves, through the real libobs;
# test_transition_scale_type covers the matrix of each scale type at a
# start. Transitions need core video, so it builds on Linux only and runs
# under xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
