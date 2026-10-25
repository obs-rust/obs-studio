# Characterization of the parameter values of libobs/graphics/effect.c
# (gs_effect_set_*, the getters, gs_effect_set_default and
# gs_effect_set_next_sampler) before their Rust port (#2409). Calls the
# exported functions through the real libobs on a hand-built
# struct gs_effect_param: no graphics context, so it runs on every platform.
# Defined in ../build/test-graph.json.
