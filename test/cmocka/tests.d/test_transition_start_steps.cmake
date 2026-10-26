# Characterization of the start checks and child handover of the
# transitions in libobs/obs-source-transition.c, moved to
# libobs/obs-source-transition-start.c and ported to rust/obs-source
# (transition_start): every gate of obs_transition_start() over every
# combination of its inputs (the first refusal, the recursion walk, the
# second admission over six modes), the duration in ns to UINT32_MAX ms,
# set_source()'s clock rule and handover, activate_transition(), the active
# index, the manual reset at 0, -0, NaN and 1, and the active child rule;
# and copies of the callers on a stand-in transition that counts each
# child's references and active references: the start from idle, to source
# A, while running and to no destination, a manual start to the running
# destination, a re-entering transition_start callback, a failed add that
# keeps the nested start's clock, the fixed duration, the manual times, the
# swap and the set. Builds the moved file into itself. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. test_transition_recursive_destination, test_transition_swap,
# test_studio_mode_swap, test_studio_mode_exit, test_saved_transition,
# test_tbar_position, test_fade_to_color_mix, test_visibility_transition and
# test_stinger_media_duration cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
