# Characterization of the audio timestamp placement of
# source_output_audio_data() in libobs/obs-source.c, moved to
# libobs/obs-source-audio-timing.c before its port: the two steps around
# audio_buf_mutex and the buffer start, every branch, the thresholds at their
# edges, the wraps, and packet sequences through a fake source. Builds the
# moved file on its own: no display and no libobs core, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. test_async_delay_audio,
# test_audio_sync_offset_change, test_media_sync_offset_restart,
# test_audio_restart_resampler_tail and test_audio_clock_drift cover the caller
# through the real libobs.
# Target and test are declared in ../build/test-graph.json.
