# The core-audio seam (libobs/obs-audio-seam.h), which the port of the audio
# tick (libobs/obs-audio.c) calls: each read against the field it wraps, each
# write read back, each lock and unlock holding and letting go of its mutex,
# and the render order, root node, buffered timestamp and task queue calls
# against the da_*/deque_* calls they copy. The accessors are hidden in
# libobs, so the test builds the seam into itself, over a stand-in core and
# stand-in sources and views.
# Target and test are declared in ../build/test-graph.d.
