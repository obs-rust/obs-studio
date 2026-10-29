# Characterization of the setup rules of libobs/obs.c, moved to
# libobs/obs-core-setup-rules.c (#2878): the reset gates, the audio
# buffering ticks, the command line's copy, the data path match, the latest
# version compare, the capture device check, the monitoring device checks and
# the frame-count rule, through the hidden functions compiled from the moved
# file; and obs_reset_video(), obs_reset_audio2(), obs_set_cmdline_args(),
# obs_remove_data_path(), obs_get_latest_input_type_id(),
# obs_set_audio_monitoring_device() and obs_get_video_frame_counts() of the
# real libobs through the public API, so the callers run against the C
# backend, the hybrid libobs and libobs-c-reference. obs_startup() needs a
# display on Linux, so the test runs under xvfb-run there.
# Target and test are declared in ../build/test-graph.json.

# The moved file is compiled into the test with ASan and UBSan (the
# "sanitizers" entry in test-graph.json). No _rust variant, as for the other
# obs-core step tests: the shims' archive member (an obs_core codegen unit)
# also holds the other obs-core ports, which need libobs and obs-util's
# platform code, so it does not link on its own, and beside libobs it would
# carry a second copy of libobs's Rust symbols. The Tier 3 parity tests
# (rust/obs-core/tests/obs_core/core_setup_rules_parity.rs) compare the shims
# with this C instead, and the caller cases here run against the hybrid
# libobs.
