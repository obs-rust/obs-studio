# Characterization of the Windows key map and key name route of
# libobs/obs-windows.c, moved to libobs/obs-windows-keys.c and ported to
# rust/obs-hotkey (windows_keys): the virtual key table, the Meta rule of the
# pressed check, the key name route with its scan code and system name gate,
# the two virtual key lookups and the combination order. Builds the moved
# file on its own: no Windows header and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. There is no _rust variant against
# libobs_rust-static: the shims' archive members pull in the rest of
# obs-hotkey, obs-data and obs-util, so it does not link on its own, as
# test_portal_shortcut_sets. The Tier 3 parity tests in rust/obs-hotkey
# compare the shims with this C instead.
# Target and test are declared in ../build/test-graph.json.
