# Characterization of the scene item visibility audio ramp in
# libobs/obs-scene-item-audio-actions.c and its core,
# libobs/obs-scene-audio-actions.c, before the port of the core: the 0/1 gain
# buffer, the actions consumed and the hides counted, sample by sample, with
# a fake scene item. Builds the two files on their own: no display and no
# libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes those C sources; the test calls only them, so
# the port can run the item side against the Rust core in the staticlib (as
# test_scene_bounds_place_rust does). test_scene_audio_actions covers the
# callers through the real libobs.
# Defined in ../build/test-graph.d.
