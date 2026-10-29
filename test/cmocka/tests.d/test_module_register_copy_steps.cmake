# Characterization of the registration size rules of libobs/obs-module.c,
# moved to libobs/obs-module-register-copy.c (#2860): the copy length for
# every size around each field, the in-size rule, the zero and too-large
# gate and the transition rule, through the four hidden functions compiled
# from the moved file; and obs_register_source_s(), obs_register_output_s(),
# obs_register_encoder_s() and obs_register_service_s() of the real libobs
# through the public API (sizes cut at and inside a field, at 0 and past
# sizeof(), and when the type data is freed), so the callers run against the
# C backend, the hybrid libobs and libobs-c-reference. obs_startup() needs a
# display on Linux, so the test runs under xvfb-run there.
# Target and test are declared in ../build/test-graph.json.

# The moved file is compiled into the test with ASan and UBSan (the
# "sanitizers" entry in test-graph.json). No _rust variant, as for
# test_module_load_steps and test_module_type_rules_steps: the shims' archive
# member (an obs_core codegen unit) also holds the other obs-core ports, which
# need libobs and obs-util's platform code, so it does not link on its own
# (test_canvas_rules_steps measured it), and beside libobs it would carry a
# second copy of libobs's Rust symbols. The Tier 3 parity tests
# (rust/obs-core/tests/obs_core/module_register_copy_parity.rs) compare the
# shims with this C instead, and the caller cases here run against the hybrid
# libobs.
