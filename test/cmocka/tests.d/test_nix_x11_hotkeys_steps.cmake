# Characterization of the X11 key names and raw button decode of
# libobs/obs-nix-x11.c, moved to libobs/obs-nix-x11-hotkeys.c: how each key is
# named (mouse button, numpad digit, fixed default name, function key, or its
# keycodes) and how one polled event is decoded (a raw button press or
# release and its slot, an unsupported button, or nothing). Builds the moved
# file on its own: no X server and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. Target and test are declared in
# ../build/test-graph.json.
