# Characterization of the X11 keysym table and mouse button answers of
# libobs/obs-nix-x11.c, moved to libobs/obs-nix-x11-input.c and ported to
# rust/obs-hotkey (nix_x11_input): the keysym of each key, the slot each mouse
# key reads with XInput, the answer (pressed in this poll or held from
# before), the commit of the poll's raw button events, and the pointer mask
# answer without XInput. Builds the moved file on its own: no X server and no
# libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# There is no _rust variant against libobs_rust-static: the shims' archive
# members pull in the rest of obs-hotkey, obs-data and obs-util (jansson,
# libuuid, GIO, and libobs's hidden hotkey seams), so it does not link on its
# own, as test_nix_x11_keys_steps. The Tier 3 parity tests in rust/obs-hotkey
# compare the shims with this C instead, and test_x11_volume_keys and
# test_x11_keymap cover the callers through the real libobs. Target and test
# are declared in ../build/test-graph.json.
