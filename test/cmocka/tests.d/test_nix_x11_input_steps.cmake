# Characterization of the X11 keysym table and mouse button answers of
# libobs/obs-nix-x11.c, moved to libobs/obs-nix-x11-input.c: the keysym of
# each key, the slot each mouse key reads with XInput, the answer (pressed in
# this poll or held from before), the commit of the poll's raw button events,
# and the pointer mask answer without XInput. Builds the moved file on its
# own: no X server and no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. Target and test are declared in
# ../build/test-graph.json.
