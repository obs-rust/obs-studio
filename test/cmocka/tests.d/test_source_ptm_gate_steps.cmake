# Characterization of the push-to-mute and push-to-talk gate of
# source_muted() in libobs/obs-source.c, moved to libobs/obs-source-ptm-gate.c
# before its port: the stop time of a held key (#2058's saturation at the
# product and the sum edges, a delay of 0, negative int64 delays as the
# setters take them), the hold strictly before the stop time, the two gates,
# every combination of enabled, user_muted and each gate's flag and key,
# releases inside and after the delay, and the stop time get_source_volume()
# shares. Builds the moved file on its own: no display and no libobs core, so
# it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them.
# test_source_audio_ranges covers the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
