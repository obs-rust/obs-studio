# Characterization of the libswresample resampler's output plan of
# libobs/media-io/audio-resampler-ffmpeg.c (the channel mask of each speaker
# layout, the output channels and planes, the input channels, the 4.1 layouts
# and the mono upmix gate), moved to libobs/media-io/audio-resampler-plan.c.
# Builds the moved file on its own: no FFmpeg header and no libobs core, so it
# runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source. test_audio_clock_drift and
# test_audio_restart_resampler_tail cover the caller through the real libobs.
# Target and test are declared in ../build/test-graph.json.
