# Characterization of load_all_callback()'s skip ladder in
# libobs/obs-module.c (the checks it makes before it opens a module), moved to
# libobs/obs-module-load-rules-skip.c and ported to rust/obs-core
# (module_load_rules). Builds the moved file on its own, with ASan and UBSan:
# no libobs core, so it runs on every platform. The function is hidden in
# libobs, so this characterizes that C source. There is no _rust variant, as
# for test_module_load_steps: the shims' archive member also holds the other
# obs-core ports, which need libobs and obs-util, so it does not link on its
# own. The Tier 3 parity tests
# (rust/obs-core/tests/obs_core/module_load_rules_parity.rs) compare the shim
# with this C instead, and test_module_load_steps, test_core_module_load and
# test_plugin_manager cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
