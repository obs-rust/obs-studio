# Characterization of the GlobalShortcuts portal client's status decisions in
# libobs/obs-nix-portal-shortcut-status.c, ported to rust/obs-hotkey
# (portal_shortcut_status): how a shortcut's description names its actions,
# the "OBS" fallback, what bind_approved() and sync_shortcuts() do after a
# bind, which shortcuts the desktop's triggers mark bound, and when the
# desktop's shortcut editor opens. Builds the moved file on its own: no GIO,
# no D-Bus and no libobs core. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. There is no _rust
# variant against libobs_rust-static: the shims' archive members pull in the
# rest of obs-hotkey, obs-data and obs-util (jansson, libuuid, GIO, and
# libobs's hidden hotkey seams), so it does not link on its own, as
# test_portal_shortcut_sets. The Tier 3 parity tests in rust/obs-hotkey
# compare the shims with this C instead, and test_portal_shortcuts covers the
# client end to end. Target and test are declared in ../build/test-graph.json.
