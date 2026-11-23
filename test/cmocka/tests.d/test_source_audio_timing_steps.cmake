# Characterization of the audio timestamp placement of
# source_output_audio_data() in libobs/obs-source.c, moved to
# libobs/obs-source-audio-timing.c and ported to rust/obs-source
# (audio_timing): the two steps around audio_buf_mutex and the buffer start,
# every branch, the thresholds at their edges, the wraps, and packet sequences
# through a fake source. Builds the moved file on its own: no display and no
# libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_async_delay_audio, test_audio_sync_offset_change,
# test_media_sync_offset_restart, test_audio_restart_resampler_tail and
# test_audio_clock_drift cover the caller through the real libobs.
# Target and test are declared in ../build/test-graph.d.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the three hidden functions and includes only their header, so it
# links the archive alone: its obs-source member needs nothing from libobs or
# the other crates. Linking the archive directly avoids exporting private
# symbols just for a test.
# The Rust-static target and test are also declared in ../build/test-graph.d.
