# Characterization of the transition settings functions of
# libobs/obs-source-transition.c before they move to their own file and are
# ported to rust/obs-source: obs_transition_is_active(), the scale type,
# alignment and size setters and getters, obs_transition_save() and the
# fixed duration switch. Pins each getter after each setter, a new
# transition's values, the getters' defaults for NULL and for a source that
# is not a transition, and every key obs_transition_save() writes, through
# the real libobs. Transitions need core video, so it builds on Linux only
# and runs under xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
