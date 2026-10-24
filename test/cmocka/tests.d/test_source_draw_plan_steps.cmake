# Characterization of the source draw decisions of libobs/obs-source.c
# (rotate_async_video(), obs_source_render_async_video() and source_render()),
# moved to libobs/obs-source-draw-plan.c and ported to rust/obs-source
# (draw_plan): the rotation translate for 0, 90, 180, 270 and other angles on
# non-square sizes, the linear sRGB flag and the non-linear alpha blend of an
# async frame, the technique of every pair of colour spaces with and without
# non-linear alpha, the white level read and the multiplier at SDR white
# levels of 80, 300 and 0, and whether source_render() converts through
# color_space_texrender. Builds the moved file into itself. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. test_async_rotation_normalize, test_filter_color_space and
# test_source_color_space_steps cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
