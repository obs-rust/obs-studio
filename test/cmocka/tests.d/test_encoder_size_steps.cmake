# Characterization of the encoder size and info choices of libobs/obs-encoder.c,
# moved to libobs/obs-encoder-size.c and ported to rust/obs-output
# (encoder_size): get_audio_info()'s default fill from the mix,
# get_video_info()'s OBS_ENCODER_CAP_SCALING override, the refusals and the
# "matches the output" rule of obs_encoder_set_scaled_size(),
# obs_encoder_scaling_enabled(), and where obs_encoder_get_width(), _height()
# and obs_encoder_get_sample_rate() take their value from, at the u32 edges.
# Builds the moved file on its own: no display and no libobs core, so it runs
# on every platform. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them. There is no _rust variant against
# libobs_rust-static: the shims' archive member also holds the rest of
# obs-output (the service lifecycle, the delay and the interleaver), which needs
# libobs's hidden output and service seams and obs-util (GIO, libuuid), so it
# does not link on its own (checked: 506 undefined references). The Tier 3
# parity tests in rust/obs-output compare the shims with this C instead.
# test_encoder_video_reset, test_encoding_lag_rescale,
# test_multitrack_encoder_override, test_encoder_fallback and test_encoder_roi
# cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
