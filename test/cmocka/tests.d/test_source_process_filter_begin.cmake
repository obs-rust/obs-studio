# obs_source_process_filter_begin and
# obs_source_process_filter_begin_with_color_space (#5831), called directly
# and from a filter that renders through them: the NULL filter and removed
# filter log lines, the bypass, the texture path and its color space, the
# zero-size skip and a texrender of another format replaced. Linux with
# OpenGL, under xvfb-run.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
