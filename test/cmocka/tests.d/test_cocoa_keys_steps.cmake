# Characterization of the macOS key map and pressed check of
# libobs/obs-cocoa.m, moved to libobs/obs-cocoa-keys.c and ported to
# rust/obs-hotkey (cocoa_keys): the modifier flag map, the two virtual key
# lookups, the mouse key range and the key range gate. Builds the moved file
# on its own: no macOS header and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. There is no _rust variant against
# libobs_rust-static: the shims' archive members pull in the rest of
# obs-hotkey, obs-data and obs-util, so it does not link on its own, as
# test_portal_shortcut_sets. The Tier 3 parity tests in rust/obs-hotkey
# compare the shims with this C instead.
# Target and test are declared in ../build/test-graph.json.
