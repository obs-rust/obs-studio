# Characterization of the encoder size and info choices of libobs/obs-encoder.c,
# moved to libobs/obs-encoder-size.c before their Rust port: get_audio_info()'s
# default fill from the mix, get_video_info()'s OBS_ENCODER_CAP_SCALING override,
# the refusals and the "matches the output" rule of obs_encoder_set_scaled_size(),
# obs_encoder_scaling_enabled(), and where obs_encoder_get_width(), _height() and
# obs_encoder_get_sample_rate() take their value from, at the u32 edges. Builds
# the moved file into itself. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them.
# test_encoder_video_reset, test_encoding_lag_rescale,
# test_multitrack_encoder_override, test_encoder_fallback and test_encoder_roi
# cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
