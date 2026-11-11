# Characterization of the video output plan in libobs/obs-video-plan.c before
# its port: the size check and alignment of obs_reset_video(), and for each
# output format the GPU conversion techniques, texel sizes, conversion planes
# and render texture format. Compiles the file into itself (C only), so it
# needs no libobs core and no display. libobs.pch.h needs the private libobs,
# libcaption and uthash headers.
# test_video_output_plan is defined in ../build/test-graph.d.
