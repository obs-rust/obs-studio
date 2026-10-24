# Characterization of the scene item names and coordinate keys of
# scene_load_item() and scene_save_item() in libobs/obs-scene.c, moved to
# libobs/obs-scene-item-names.c before their port: the scale filter, blend
# method and blend type names each value loads from (astrcmpi: every case,
# unknown names, a trailing space, bytes above 0x7F) and is saved under
# (values outside each enum), and which keys the position, scale and bounds
# load from, for every pattern of the four relative keys, with and without
# absolute coordinates. Builds the moved file on its own: no display and no
# libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them. The
# real-libobs scene tests cover the caller.
# Target and test are declared in ../build/test-graph.json.
