# Characterization of the GPU rescale mix decisions of libobs/obs-encoder.c,
# moved to libobs/obs-encoder-gpu-rescale.c before their port to
# rust/obs-output (encoder_gpu_rescale, #3137): the gates of
# maybe_set_up_gpu_rescale() (no media, OBS_SCALE_DISABLE, nothing requested),
# the target size, format, colorspace and range (the encoder's value or the
# video output's), the match of a mix on each of its conditions, and the
# release step and reference count of maybe_clear_encoder_core_video_mix(), at
# the u32 edges and with enum values the C does not range-check. Builds the
# moved file on its own: no display and no libobs core calls, so it runs on
# every platform. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them. test_encoding_lag_rescale and
# test_output_rescale cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
