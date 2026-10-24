# Characterization of the volume decisions of get_source_volume() and
# apply_audio_volume() in libobs/obs-source.c, moved to
# libobs/obs-source-volume.c before their port to rust/obs-source
# (volume_apply): the gate over every combination of its six flags and stop
# times around the clock (through obs-source-ptm-gate.c's helpers), the 0/1
# snap at 0 +- 0.0001 and 1 +- 0.0001 and the float neighbours of each edge,
# -0, NaN and +-inf, the action-due compare at the threshold - 1, at it and
# + 1 and where the u64 sum wraps, and skip, clear or multiply for each volume
# with every mixer mask edge (0, 1, 0x3f, 0x40 and above, UINT32_MAX).
# get_source_volume() and apply_audio_volume() are copied onto a stand-in
# source, with the memset and the multiply recorded. Builds the moved files
# on their own: no display, so it runs on every platform, and links libm
# except on MSVC. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them. test_source_audio_ranges,
# test_source_volume_mix, test_audio_capture_mix_race and the scene item
# audio action tests cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
