# Characterization of the libswresample resampler's decisions of
# libobs/media-io/audio-resampler-ffmpeg.c (the sample format, the mono upmix
# row, the clock drift compensation loop and the output frame padding), moved
# to libobs/media-io/audio-resampler-drift.c and ported to rust/obs-media-io
# (audio_resampler_drift). Builds the moved file on its own: no FFmpeg header
# and no libobs core, so it runs on every platform. The functions are hidden
# in libobs, so this characterizes that C source. There is no _rust variant
# against libobs_rust-static: the shims' archive member also holds the rest of
# obs-media-io, which needs obs-util (bcrash and libuuid), so it does not link
# on its own (checked: 238 undefined references). The Tier 3 parity tests in rust/obs-media-io compare the shims
# with this C instead. test_audio_clock_drift and
# test_audio_restart_resampler_tail cover the caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.
