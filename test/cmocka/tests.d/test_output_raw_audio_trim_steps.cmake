# Characterization of the two sample-rate conversions of prepare_audio() in
# libobs/obs-output.c, moved to libobs/obs-output-raw-audio-trim.c before their
# port to rust/obs-output (raw_audio_trim, #3479): the length of a raw-audio
# packet that starts before video and whether it is dropped, and the cut in
# frames of one that is kept, at 1 Hz to UINT32_MAX Hz, around the video start,
# at the top of the clock and with the most frames. Every case is also checked
# against the expressions obs-output.c had before the move. A zero rate, which
# the port changes on purpose, is not covered for the drop. Builds the moved
# file and obs-output-raw-audio.c (for obs_output_raw_audio_ends_before()) on
# their own, so it runs on every platform. The functions are hidden in libobs,
# so this characterizes that C source; the test calls only them. The Tier 3
# parity tests in rust/obs-output compare the shims with this C.
# test_output_raw_audio_steps, test_output_before_video and
# test_raw_output_video_reset cover the caller.
# Target and test are declared in ../build/test-graph.json.
