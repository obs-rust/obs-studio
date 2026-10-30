# Characterization of the two mix kernels of scene_audio_render() in
# libobs/obs-scene.c, moved to libobs/obs-scene-audio-mix.c before their port
# to rust/obs-scene (audio_mix, #3079): the plain add at the start, inside and
# at the end of the tick for counts 0, 1, 3 and a whole tick, the add through
# the gain buffer (gains 0, 1, 0.5, 2, -1, a hide mid-tick at pos 0 and 301,
# the gain indexed like the input), IEEE NaN, infinity and -0.0 in each
# operand, a count of 0 and a second add. Every product is exact, so the
# expectations hold whether or not the compiler fuses the multiply and add.
# Builds the moved file on its own: no display and no libobs core calls, so it
# runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. The Tier 3 parity
# tests in rust/obs-scene compare the shims with this C.
# Target and test are declared in ../build/test-graph.json.
