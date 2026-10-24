# Characterization of the canvas texture draw and display clear decisions of
# libobs/obs.c (obs_render_canvas_texture_internal()) and libobs/obs-display.c
# (render_display_begin()), moved to libobs/obs-canvas-draw.c before their port
# to rust/obs-core (canvas_draw): the early return with and without a mix and a
# rendered texture, the technique for every pair of colour spaces and spaces
# outside the enum, the white level read and the multiplier at SDR white levels
# of 80, 300 and 0 and any float, the clear colour conversion for both host
# answers and every space, and the clear flags and solid colour workaround.
# Builds the moved file on its own: no display and no libobs core, so it runs on
# every platform. The functions are hidden in libobs, so this characterizes that
# C source; the test calls only them and obs_source_draw_technique_name().
# test_canvas_flags, test_scene_private_canvas, test_canvas_destroy_scene_refs
# and test_shutdown_canvas_weak_ref cover the canvases through the real libobs.
# Target and test are declared in ../build/test-graph.json.
