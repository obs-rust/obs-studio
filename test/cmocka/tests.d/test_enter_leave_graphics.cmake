# Characterization of obs_enter_graphics and obs_leave_graphics before their
# Rust swap (#5459): no-ops before the first video reset; after it, enter makes
# the core's graphics context current on the calling thread, entries are
# counted, and another thread waits until the holder leaves. Needs a display
# and OpenGL. Registered once in build/test-graph.json.
