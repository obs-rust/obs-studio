# Characterization of the async frame copy plan and cache decisions of
# libobs/obs-source.c, moved to libobs/obs-source-frame-copy.c before its
# port: the copies of copy_frame_data() for every video format and values
# outside the enum (P216 and P416 copy nothing), odd heights and the uint32_t
# wrap of (height + 1) / 2, equal, smaller, larger and 0 line sizes per plane,
# the colour range, the queue limit of cache_video() at MAX_ASYNC_FRAMES - 1,
# at it and + 1, async_texture_changed(), clean_cache()'s drop at
# MAX_UNUSED_FRAME_DURATION, the full-range rule of obs_source_output_video()
# and its *2() variant, and preload_frame_changed(). Builds the moved file on
# its own: no display and no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. test_video_frame, the media restart tests, the v4l2
# and PipeWire camera tests and the deinterlace tests cover the callers
# through the real libobs.
# Target and test are declared in ../build/test-graph.json.
