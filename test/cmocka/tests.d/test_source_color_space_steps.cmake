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

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the two hidden functions and includes only their header and
# graphics/graphics.h (for enum gs_color_space), so it links the archive alone:
# its obs-source member needs nothing from libobs or the other crates. Linking
# the archive directly avoids exporting private symbols just for a test.
# Rust-static target and test are declared in ../build/test-graph.json.
