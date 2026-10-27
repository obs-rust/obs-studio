# Characterization of the load, scale type and swap checks of the
# transitions in libobs/obs-source-transition.c, moved to
# libobs/obs-source-transition-load.c and ported to rust/obs-source
# (transition_load): the type check of transition_valid(), the scale type
# rule over the edges of int64_t, the source steps of obs_transition_load(),
# the swap gate and copy_source_state()'s recursion refusal over every
# combination of their inputs; and copies of the callers on a stand-in
# transition that counts each child's references and active references: the
# loaded fields (truncated and out of range), a missing, unknown and
# recursive source, a load onto a filled slot A, the swap with itself and
# every slot state of the swap, and the getters on a source that is not a
# transition. Builds the moved files into itself. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_transition_load_release, test_transition_scale_type,
# test_transition_swap, test_transition_swap_active and
# test_transition_swap_cycle cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
