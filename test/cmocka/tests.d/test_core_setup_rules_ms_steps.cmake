# Characterization of the audio buffering in milliseconds of libobs/obs.c
# (obs_reset_audio2() and obs_get_audio_info2()), moved to
# libobs/obs-core-setup-rules-ms.c and ported to rust/obs-core
# (core_setup_rules). Builds the moved file on its own, with ASan and UBSan:
# no libobs core, so it runs on every platform. The function is hidden in
# libobs, so this characterizes that C source. There is no _rust variant, as
# for test_core_setup_rules_steps: the shims' archive member also holds the
# other obs-core ports, which need libobs and obs-util, so it does not link on
# its own. The Tier 3 parity tests
# (rust/obs-core/tests/obs_core/core_setup_rules_parity.rs) compare the shims
# with this C instead, and test_core_setup_rules_steps covers the callers
# through the real libobs. Target and test are declared in
# ../build/test-graph.json.
