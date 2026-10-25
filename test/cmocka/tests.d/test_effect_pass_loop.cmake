# Characterization of the technique and pass loop of
# libobs/graphics/effect-passes.c (gs_effect_loop, the technique and pass
# begin/end functions, gs_effect_update_params and the parameter upload)
# before its Rust port. Compiles the C file into itself with recording stubs
# for the device functions and gs_get_effect: no graphics context. Linux only
# (--gc-sections), as test_audio_buffering_subsample. The render tests cover
# the loop through the real libobs.
# Defined in ../build/test-graph.json.
