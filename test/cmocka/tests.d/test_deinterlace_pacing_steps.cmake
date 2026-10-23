# Characterization of the 2x deinterlace frame pacing of
# deinterlace_get_closest_frames() and ready_deinterlace_frames() in
# libobs/obs-source-deinterlace.c, moved to
# libobs/obs-source-deinterlace-pacing.c before its port: the unbuffered hold
# and its jump threshold, the "don't skip ahead" guard, the half duration, the
# buffered offset and the unbuffered re-anchor (#395), every branch, the
# thresholds at their edges, the wraps, and unbuffered sequences at 25, 29.97,
# 50 and 59.94 frames a second into 30 and 60 fps. Builds the moved file on its
# own: no display and no libobs core, so it runs on every platform. The
# functions are hidden in libobs, so this characterizes that C source; the test
# calls only them. test_deinterlace_2x_field_order covers the caller through
# the real libobs.
# Target and test are declared in ../build/test-graph.json.
