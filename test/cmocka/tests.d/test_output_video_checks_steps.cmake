# Characterization of the output video size, conversion and capture checks of
# libobs/obs-output.c, moved to libobs/obs-output-video-checks.c and ported to
# rust/obs-output (video_checks): the first mixer of a mixer mask, where
# obs_output_get_width2() and _height2() take an axis from, the flag and
# presence checks of can_begin_data_capture(), the delay-active and active gates
# of obs_output_can_begin_data_capture(), the plan of
# obs_output_get_video_conversion() and the congestion clamp (NaN passes
# through). Builds the moved file on its own: no display and no libobs core, so
# it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. There is no _rust
# variant against libobs_rust-static: the shims' archive member also holds the
# rest of obs-output (the service lifecycle, the delay and the interleaver),
# which needs libobs's hidden output and service seams and obs-util (GIO,
# libuuid), so it does not link on its own (checked: 506 undefined references).
# The Tier 3 parity tests in rust/obs-output compare the shims with this C
# instead. test_raw_output_video_reset, test_scaled_mix_alpha,
# test_texture_encoder_raw_format, test_output_start_twice,
# test_output_failed_start_encoders, test_output_null_id and
# test_encoding_lag_rescale cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
