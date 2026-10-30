# Characterization of the volume multiply kernels of libobs/obs-source.c,
# moved to libobs/obs-source-volume-mul.c before their port to
# rust/obs-source (volume_mul, #3080): one volume over 1, 2 and 8 channels
# of a mix laid out as allocate_audio_output_buffer() lays it out, counts
# from 0 to past a tick, a volume per frame (a fade and an action's step)
# channel by channel, and NaN, infinities, -0.0, the smallest denormal and
# a huge value in either operand; the samples after the count keep a
# canary. A NaN sample times a NaN volume is not covered: its payload
# depends on how the C is compiled. Builds the moved file on its own: no
# display and no libobs core calls, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. The Tier 3 parity tests in rust/obs-source compare
# the shims with this C.
# Target and test are declared in ../build/test-graph.json.
