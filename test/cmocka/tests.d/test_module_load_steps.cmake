# Characterization of the module load rules of libobs/obs-module.c, moved to
# libobs/obs-module-load-rules.c (#2801): the open gates, the required
# exports, the version rule, the safe and disabled list rules, the core and
# legacy types and the routing of load_all_callback(), through the hidden
# functions compiled from the moved file; and obs_open_module(),
# obs_get_module_allow_disable(), obs_is_core_module() and
# obs_is_legacy_module() of the real libobs through the public API, so the
# callers run against the C backend, the hybrid libobs and
# libobs-c-reference. obs_startup() needs a display on Linux, so the test
# runs under xvfb-run there.
# Target and test are declared in ../build/test-graph.json.

# The moved file is compiled into the test with ASan and UBSan (the
# "sanitizers" entry in test-graph.json). No _rust variant, as for
# test_canvas_rules_steps and test_module_type_rules_steps: the shims' archive
# member (an obs_core codegen unit) also holds the other obs-core ports, which
# need libobs and obs-util's platform code, so it does not link on its own
# (test_canvas_rules_steps measured it), and beside libobs it would carry a
# second copy of libobs's Rust symbols. The Tier 3 parity tests
# (rust/obs-core/tests/obs_core/module_load_rules_parity.rs) compare the shims
# with this C instead, and the caller cases here run against the hybrid libobs.
