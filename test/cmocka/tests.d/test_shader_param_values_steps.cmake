# Characterization of the number reads of the shader parser,
# libobs/graphics/shader-parser.c, moved to
# libobs/graphics/shader-param-values.c before their port to rust/obs-graphics
# (shader_param_values, #3468): an array count, an int default value and a
# float default value. Builds the moved file on its own, with ASan and UBSan:
# no display and no graphics context, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. Past INT_MAX the C result depends on the width of
# long, and those cases are pinned per width. There is no _rust variant: the
# port saturates there instead (the intentional difference of #3468, the
# effect parser's rule of #3464), so the edge cases would fail against the
# shims. The Tier 3 parity tests
# (rust/obs-graphics/tests/shader/shader_param_values_parity.rs) compare the
# shims with this C on every value inside int range, the Tier 1 tests mirror
# this file's cases, and test_shader_number_defaults covers the callers
# through the real libobs. Target and test are declared in
# ../build/test-graph.json.
