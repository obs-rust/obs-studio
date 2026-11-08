# The transition seam's render, audio and lifecycle accessors
# (libobs/obs-source-transition-seam.h), which the transition render, audio,
# init and free ports call: the context data, the core's graphics and
# transparent texture, a child's two audio flags and first output buffer,
# and init, destroy and init again of both transition mutexes. The accessors
# are hidden in libobs, so the test builds the seam (and the load checks
# transition_valid() asks) into itself, over a stand-in core and stand-in
# sources.
# Target and test are declared in ../build/test-graph.json.
