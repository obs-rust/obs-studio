# Characterization of the X11 key names and raw button decode of
# libobs/obs-nix-x11.c, moved to libobs/obs-nix-x11-hotkeys.c and ported to
# rust/obs-hotkey (nix_x11_hotkeys): how each key is named (mouse button,
# numpad digit, fixed default name, function key, or its keycodes) and how one
# polled event is decoded (a raw button press or release and its slot, an
# unsupported button, or nothing). Builds the moved file on its own: no X
# server and no libobs core, so it runs on every platform. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. There is no _rust variant against libobs_rust-static: the shims'
# archive members pull in the rest of obs-hotkey, obs-data and obs-util
# (jansson, libuuid, GIO, and libobs's hidden hotkey seams), so it does not
# link on its own, as test_nix_x11_input_steps. The Tier 3 parity tests in
# rust/obs-hotkey compare the shims with this C instead, and
# test_x11_hotkey_layout and test_hotkey_translations cover the callers
# through the real libobs. Target and test are declared in
# ../build/test-graph.json.
