# The transition seam's callback, signal and video-time accessors
# (libobs/obs-source-transition-seam.h), which the transition ports call:
# both callback calls with and without their callbacks, the two signals each
# dosignal emits (NULL names and a private source skipped), and the video
# time against the frame time. The accessors are hidden in libobs, so the
# test builds the seam (and the load checks transition_valid() asks) into
# itself, over a stand-in core and stand-in transitions.
# Target and test are declared in ../build/test-graph.d.
