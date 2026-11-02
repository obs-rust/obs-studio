# Characterization of the two sample-rate conversions of the encoder's audio
# start in libobs/obs-encoder.c, moved to libobs/obs-encoder-audio-start-frames.c
# before their port to rust/obs-output (encoder_audio_start_frames, #3487):
# calc_offset_size()'s offset in frames and buffer_audio()'s packet length in
# nanoseconds, at 1 Hz to UINT32_MAX Hz, with the most frames, with offsets whose
# product with the rate passes 2^64, and through the rules they feed
# (obs_encoder_audio_start_packet() and _offset_size()). Every case is also
# checked against the expressions obs-encoder.c had before the move. A zero
# rate, which the port changes on purpose, is not covered. Builds the moved
# file and obs-encoder-audio-start.c on their own, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. The Tier 3 parity tests in rust/obs-output
# compare the shims with this C. test_encoder_audio_start_steps covers the
# caller.
# Target and test are declared in ../build/test-graph.json.
