# Characterization of the per-frame video output step decisions of
# libobs/obs-video.c, moved to libobs/obs-video-output-step.c before their port
# to rust/obs-core (video_output_step): update_active_state()'s clears for every
# combination of the was flags and the two active counters, what
# stage_output_texture() stages with and without GPU conversion for each channel
# count, queue_frame()'s duplicate, copy or swap for each queue state and repeat
# count with the separate UV copy and the count left, render_video()'s steps for
# each combination, and output_frame()'s texture ring (the wrap at NUM_TEXTURES),
# download and output. Counts and ring values outside what libobs sets are
# characterized, not endorsed. Builds the moved file on its own: no display and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_video_output_formats, test_raw_output_video_reset,
# test_encoding_lag_rescale, test_scaled_mix_alpha, test_video_output_plan and
# the texture encoder tests cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
