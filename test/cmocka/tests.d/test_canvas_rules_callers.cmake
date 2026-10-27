# Characterization of the callers of the canvas flag rules (libobs/obs-canvas.c,
# whose decisions moved to libobs/obs-canvas-rules.c and rust/obs-core
# canvas_rules) through the real libobs: the flags each constructor keeps
# (the saved int64 truncated before the mask), what obs_save_canvas() writes
# or refuses, obs_canvas_reset_video() refusing no video info, the main canvas
# and active video, obs_canvas_set_name()'s refusals and its public and
# private renames, removing twice and the main canvas, the SCENE_REF rule for
# a scene, a group and a plain source and the group walk, the main canvas's
# global source_rename, and a channel_change handler replacing the source.
# Needs a display and OpenGL; public API only, so it runs against libobs, the
# hybrid libobs and libobs-c-reference. test_canvas_rules_steps pins the rules
# on their own. Target and test are declared in ../build/test-graph.json.
