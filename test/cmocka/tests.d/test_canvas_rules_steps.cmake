# Characterization of the canvas flag rules and setter checks of
# libobs/obs-canvas.c, moved to libobs/obs-canvas-rules.c before their port to
# rust/obs-core (canvas_rules): the copied flag values, the main canvas's flags
# and the public mask over every bit, obs_load_canvas()'s int64 truncation
# before the mask, every flag rule (view type, mix_audio, scene references,
# save, reset, global signals, rename, remove) over every combination of the
# known flags, REMOVED and an unknown bit, the main, program, preview and
# device canvases, the public canvas rule, the video info checks, the channel
# bound, the empty name and removing twice. Builds the moved file into itself,
# with no display and no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the test
# calls only them. test_canvas_flags, test_scene_private_canvas,
# test_canvas_destroy_scene_refs, test_shutdown_canvas_weak_ref and
# test_scene_canvas_characterization cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.

# No _rust variant, as for test_canvas_draw_steps: linking this test against
# libobs_rust-static fails with 401 undefined references (108 symbols), because
# the shims' archive member (an obs_core codegen unit) also holds the other
# obs-core ports, which need libobs and obs-util's platform code. The Tier 3
# parity tests (rust/obs-core/tests/obs_core/canvas_rules_parity.rs) compare
# the shims with this C instead.
