# obs_source_skip_video_filter (#5827), called directly and from a filter
# that skips itself: a NULL filter logs one LOG_DEBUG line; over an async
# parent it draws the current frame (also when the parent is custom draw
# without video_render); with deinterlacing on it draws through the
# deinterlacer. Linux with OpenGL, under xvfb-run. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
