# Characterization of obs_transition_load() of libobs/obs-source-transition.c
# before it moves to its own file and is ported to rust/obs-source. Pins each
# field it writes from saved data (missing keys and out-of-range values
# included), the three outcomes of the slot A lookup and their logs, and the
# release of the source slot A held, through obs_load_sources() and the real
# libobs; test_transition_load_release covers that release while shown.
# Transitions need core video, so it builds on Linux only and runs under
# xvfb-run; without a display the cases skip.
# Target and test are declared in ../build/test-graph.json.
