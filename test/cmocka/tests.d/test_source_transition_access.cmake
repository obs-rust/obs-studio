# Characterization of the transition source getters and manual controls of
# libobs/obs-source-transition.c before they move to their own file and are
# ported to rust/obs-source: obs_transition_get_source(),
# obs_transition_get_active_source(), obs_transition_set_manual_torque() and
# obs_transition_set_manual_time(). Pins the slot and the reference each
# getter returns, their refusals and logs, the torque and clamp the tick
# moves the manual value with, and the reset to slot A at a manual time of
# 0, through the real libobs. Transitions need core video, so it builds on
# Linux only and runs under xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
