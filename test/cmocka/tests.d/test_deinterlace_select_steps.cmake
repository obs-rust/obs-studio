# Characterization of the deinterlace frame selection of first_frame(),
# ready_deinterlace_frames(), deinterlace_get_closest_frames() and
# deinterlace_update_async_video() in libobs/obs-source-deinterlace.c, moved to
# libobs/obs-source-deinterlace-select.c before its port to rust/obs-source
# (deinterlace_select): the first frame with one and two queued, the unbuffered
# drops and the "don't skip ahead" guard, the buffered walk (out of bounds,
# FRAME_TOLERANCE, the pairwise drop, the two-or-fewer exit and its #2009
# exception, the MAX_TS_VAR re-anchor) at its edges, for queues of 0 to 5, 30
# and 33 frames, the wraps, the take and the texture update. The callers are
# copied onto a stand-in source with every erase, release and prev_frame write
# recorded. Builds the moved file and obs-source-deinterlace-pacing.c, which it
# calls, into the test. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them.
# test_deinterlace_2x_field_order, test_media_restart_stale_frame and
# test_media_restart_first_frame cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
