# Characterization of the frame pacing decisions of video_sleep() in
# libobs/obs-video.c, moved to libobs/obs-video-pacing.c before their port to
# rust/obs-core (video_pacing, #2738): the frame time step after a sleep that
# reached the next frame time and after a late wake-up (whole intervals, at
# least one, a negative difference as one interval, large jumps near the int
# count limit), and when a ready encoder's group gets its start timestamp.
# Counts outside what libobs sets are characterized, not endorsed. Builds the
# moved file on its own: no display and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. test_video_output_formats,
# test_raw_output_video_reset, test_encoding_lag_rescale,
# test_encoder_group_null, test_encoder_lifecycle_steps,
# test_encoder_audio_start_steps and test_media_restart_stale_frame cover the
# caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.
