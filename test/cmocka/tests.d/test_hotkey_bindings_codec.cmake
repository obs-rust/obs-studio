# Characterization of the hotkey binding codec in
# libobs/obs-hotkey-bindings.c before its port (#1621). Unlike
# test_hotkey_load, it links the real libobs and goes through the public
# hotkey API only, so it also covers the Rust lane.
# obs_startup needs an X display on Linux.
# Defined in ../build/test-graph.d.
