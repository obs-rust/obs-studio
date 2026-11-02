# Characterization of the number reads of the effect parser,
# libobs/graphics/effect-parser.c, moved to
# libobs/graphics/effect-param-numbers.c and ported to rust/obs-graphics
# (effect_param_types, #3464): an int default value and an array count. Builds
# the moved file on its own, with ASan and UBSan: no display and no graphics
# context, so it runs on every platform. The functions are hidden in libobs,
# so this characterizes that C source. There is no _rust variant: past INT_MAX
# the C result depends on the width of long, and the port saturates instead
# (the intentional difference of #3464), so the edge cases would fail against
# the shims. The Tier 3 parity tests
# (rust/obs-graphics/tests/shader/effect_param_numbers_parity.rs) compare the
# shims with this C on every value inside int range, the Tier 1 tests mirror
# this file's cases, and test_effect_number_defaults covers the callers through
# the real libobs. Target and test are declared in ../build/test-graph.json.
