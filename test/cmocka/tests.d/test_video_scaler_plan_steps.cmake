# Characterization of the swscale scaler's decisions of
# libobs/media-io/video-scaler-ffmpeg.c (the scale flags, the coefficient set,
# the range, the plane heights and the plane copy), moved to
# libobs/media-io/video-scaler-plan.c and ported to rust/obs-media-io
# (video_scaler_plan). Builds the moved file on its own: no FFmpeg header and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source. There is no _rust variant
# against libobs_rust-static: the shims' archive member also holds the rest of
# obs-media-io, which needs obs-util (bcrash and libuuid), so it does not link
# on its own. The Tier 3 parity tests in rust/obs-media-io compare the shims
# with this C instead. test_video_io and test_video_io_hidden cover the caller
# through the real libobs. Target and test are declared in
# ../build/test-graph.json.
