# Characterization of the time of each audio sample a transition mixes,
# get_sample_time() of libobs/obs-source-transition.c, moved to
# libobs/obs-source-transition-sample-time.c before its port to
# rust/obs-source (transition_sample_time, #3481): sample 0 at the block's
# time, every sample of a block at 1 Hz to 192 kHz, the last sample at
# 44100, 48000 and 96000 Hz, the floored offset, manual mode, other modes,
# a block before, straddling and after the transition, a duration of 0,
# and the uint64_t sum that wraps. A sample rate of 0 is not covered: the C
# divides by zero. Builds the moved file and the clock it asks
# (libobs/obs-source-transition-clock.c) into itself. The functions are
# hidden in libobs, so this characterizes that C source; the test calls
# only them. test_transition_audio_rescale and
# test_group_hide_transition_audio cover the mix through the real libobs.
# Target and test are declared in ../build/test-graph.json.
