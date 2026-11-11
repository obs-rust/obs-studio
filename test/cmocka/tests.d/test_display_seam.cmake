# The display seam (libobs/obs-display-seam.h), which the port of the display
# (libobs/obs-display.c) calls: each read against the field it wraps, each
# write read back, each lock and unlock holding and letting go of its mutex,
# the draw callback calls against the da_* calls they copy, and the display
# list links. The accessors are hidden in libobs, so the test builds the seam
# into itself, over a stand-in core and stand-in displays.
# Target and test are declared in ../build/test-graph.d.
