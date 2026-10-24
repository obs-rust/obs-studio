# Characterization of the audio window decisions of scene_audio_render() in
# libobs/obs-scene.c, moved to libobs/obs-scene-audio-window.c before their
# port: the source each item is read from for every combination of
# visibility and running transition, the tick's earliest timestamp over item
# sets (pending items, hidden items, no timestamps), each item's window at
# frame AUDIO_OUTPUT_FRAMES - 1, at it and + 1 for the rates OBS offers, a
# timestamp before the minimum (the uint64_t wrap), the uint64_t and rate
# edges, and every skip rule. Builds the moved file on its own: no display and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them. The
# real-libobs scene audio tests cover the caller.
# Target and test are declared in ../build/test-graph.json.
