# Characterization of the Wayland key names of libobs/obs-nix-wayland.c,
# moved to libobs/obs-nix-wayland-key-names.c before their port: how each key
# is named (mouse button, numpad digit, fixed default name, function key, or
# its keysym), over every key of the table. Builds the moved file on its own:
# no compositor and no libobs core, so it runs on every platform. The
# function is hidden in libobs, so this characterizes that C source; the test
# calls only it. No test reaches the caller through the real libobs: the
# Tier 3 parity test in rust/obs-hotkey compares the port with this C.
# Target and test are declared in ../build/test-graph.json.
