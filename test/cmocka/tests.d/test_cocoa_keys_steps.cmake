# Characterization of the macOS key map and pressed check of
# libobs/obs-cocoa.m, moved to libobs/obs-cocoa-keys.c before their port: the
# modifier flag map, the two virtual key lookups, the mouse key range and the
# key range gate. Builds the moved file on its own: no macOS header and no
# libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# Target and test are declared in ../build/test-graph.json.
