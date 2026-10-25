# Characterization of the encoder audio start alignment and the setter gates of
# libobs/obs-encoder.c, moved to libobs/obs-encoder-audio-start.c and ported to
# rust/obs-output (encoder_audio_start): the group start gate, the wait for each paired audio encoder and
# the pts steps of receive_video() and send_audio_data(); buffer_audio()'s
# branches and its plan once the video has started (wait, truncate, clear,
# start from the buffer); calc_offset_size(), start_from_buffer(),
# push_back_audio() and the send loop of receive_audio(); and the first
# refusal and the divisor rule of obs_encoder_set_gpu_scale_type() and
# obs_encoder_set_frame_rate_divisor(). buffer_audio() is also copied onto a
# stand-in encoder that counts the buffered bytes, so the start is pinned end
# to end against a video start (before, at and after it by one sample and one
# packet, a packet ending exactly at it, the unpaired start, buffered audio
# below, at and above the offset, blocksizes 4 and 8). Builds the moved file
# on its own: no display and no libobs core, so it runs on every platform.
# The functions are hidden in libobs, so this characterizes that C source;
# the test calls only them. There is no _rust variant against
# libobs_rust-static: the shims' archive member also holds the rest of
# obs-output (the service lifecycle, the delay and the interleaver), which
# needs libobs's hidden output and service seams and obs-util, so it does not
# link on its own, as for test_encoder_size_steps. The Tier 3 parity tests in
# rust/obs-output compare the shims with this C instead. test_output_interleave_e2e,
# test_output_interleave_restart, test_output_delay_e2e,
# test_encoder_error_stop, test_encoding_lag_rescale, test_encoder_video_reset
# and test_multitrack_encoder_override cover the callers through the real
# libobs.
# Target and test are declared in ../build/test-graph.json.
