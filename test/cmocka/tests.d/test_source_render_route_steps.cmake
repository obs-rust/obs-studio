# Characterization of the source and filter render routing of
# libobs/obs-source.c (source_render_video(), obs_source_main_render(),
# can_bypass(), obs_source_process_filter_begin_with_color_space(),
# obs_source_process_filter_tech_end() and obs_source_skip_video_filter()),
# moved to libobs/obs-source-render-route.c: the target and parent check
# (#2177), the "no video", async update and inactive checks, every draw route
# with the values each one reads, the default-effect, sRGB and custom draw
# rules, the direct-render bypass for every flag, wiring and pair of colour
# spaces, the skip at zero size, the texrender format check and the technique
# name. Builds the moved file into itself. The functions are hidden in libobs,
# so this characterizes that C source; the test calls only them.
# test_filter_color_space, test_filter_skip_removed and the other filter tests
# cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
