# Characterization of the task queue's route and the video level checks of
# libobs/obs.c (obs_queue_task(), obs_in_task_thread() and
# obs_set_video_levels()), moved to libobs/obs-core-setup-rules-task.c and
# ported to rust/obs-core (core_setup_rules). Builds the moved file on its
# own, with ASan and UBSan: no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source. There
# is no _rust variant, as for test_core_setup_rules_steps: the shims' archive
# member also holds the other obs-core ports, which need libobs and obs-util,
# so it does not link on its own. The Tier 3 parity tests
# (rust/obs-core/tests/obs_core/core_setup_rules_parity.rs) compare the shims
# with this C instead, and test_task, test_video_levels_range and
# test_core_setup_rules_steps cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
