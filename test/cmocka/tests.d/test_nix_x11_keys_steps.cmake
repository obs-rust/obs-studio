# Characterization of the X11 key map lookups of libobs/obs-nix-x11.c, moved
# to libobs/obs-nix-x11-keys.c and ported to rust/obs-hotkey (nix_x11_keys):
# the base keysym lookup and its right modifier fallback, fill_keycodes()'s
# row rule, key_pressed()'s decision, the keycode lookup, the column-major
# virtual key search and the virtual key of a key. Builds the moved file on
# its own: no X server and no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. There is no _rust variant against libobs_rust-static:
# the shims' archive members pull in the rest of obs-hotkey, obs-data and
# obs-util (jansson, libuuid, GIO, and libobs's hidden hotkey seams), so it
# does not link on its own, as test_portal_shortcut_sets. The Tier 3 parity
# tests in rust/obs-hotkey compare the shims with this C instead, and
# test_x11_hotkey_layout covers the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
