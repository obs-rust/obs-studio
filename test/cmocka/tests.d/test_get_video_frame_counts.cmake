# Characterization of obs_get_video_frame_counts before its Rust swap (#5443):
# 0/0 before the first video reset, the main mix's video_t counts after it,
# plus those of an encoder-only (GPU rescale) mix of the main view, never a
# mix of another view; either output may be NULL, and it changes neither the
# counters nor the mix list. Needs a display and OpenGL. Registered once in
# build/test-graph.json.
