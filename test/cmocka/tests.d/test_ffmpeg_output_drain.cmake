# Stopping a Custom FFmpeg Output lost the frames its encoders held back
# (B-frames, lookahead) and the packets not yet written. Builds the real
# obs-ffmpeg-output.c into the test and wraps avcodec_send_frame() with the
# GNU linker's --wrap to count the frames encoded. Needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
