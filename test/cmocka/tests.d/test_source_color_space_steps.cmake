# Characterization of the colour space query routing of
# obs_source_get_color_space() in libobs/obs-source.c, moved to
# libobs/obs-source-color-space.c before its port: every route for every
# combination of the source's type, VIDEO and ASYNC flags, data, enabled,
# filter parent and filter target (#1493's filter rule included), and the
# async pick from the preferred spaces (none, a match first, last or in the
# middle, no match, values outside the enum). Builds the moved file on its
# own: no display and no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the test
# calls only them. test_filter_color_space covers the caller through the real
# libobs.
# Target and test are declared in ../build/test-graph.json.
