# Characterization of the async video frame pacing of get_closest_frame() and
# ready_async_frame() in libobs/obs-source.c, moved to
# libobs/obs-source-async-pacing.c before its port: the first-frame rule, the
# unbuffered newest-frame rule, the timing jump at MAX_TS_VAR, the catch-up walk
# with its 2 ms duplicate guard and MAX_TS_VAR re-anchor, the last_frame_ts
# seed, the wraps, and buffered and unbuffered sequences, through a model of
# the async_frames queue. Builds the moved file on its own: no display and no
# libobs core, so it runs on every platform. The function is hidden in libobs,
# so this characterizes that C source; the test calls only it.
# test_media_restart_stale_frame, test_media_restart_first_frame,
# test_async_delay_audio and the v4l2 tests cover the caller through the real
# libobs.
# Target and test are declared in ../build/test-graph.json.
