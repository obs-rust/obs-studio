# The deinterlace seam (libobs/obs-source-deinterlace-seam.h), which the port
# of the rest of libobs/obs-source-deinterlace.c calls: each read against the
# field it wraps, each write read back, the lock and unlock of async_mutex,
# the queue calls against the da_* calls they copy, the texture, texrender
# and effect slots as the fields' own storage, and convert_video_space()
# against the inline. The accessors are hidden in libobs, so the test builds
# the seam into itself, over a stand-in core and stand-in sources.
# Target and test are declared in ../build/test-graph.json.
