# Characterization of the audio input steps of process_audio() and
# reset_resampler() in libobs/obs-source.c, moved to
# libobs/obs-source-audio-input.c before its port: the resampler restart
# on a discontinuity (#546) at the threshold, both ways and with no end
# set; each reset trigger alone; the clock drift fix's "no resampler
# needed" rule; the balance gate at its edges, NaN and out-of-range
# balances; the Force Mono gate; the sine, square and linear laws bit for
# bit and an unknown law; the downmix of 2, 6 and 8 channels and of no
# frames. Builds the moved file into itself. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_source_audio_balance_mono, test_audio_restart_resampler_tail and
# test_audio_clock_drift cover the caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.
