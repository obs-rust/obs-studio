# A Custom FFmpeg Output recording WebM/VP9 from the BGRA colour format
# failed to open the encoder (obsproject/obs-studio#13785). Builds the real
# obs-ffmpeg-output.c into the test, so the obs-ffmpeg module and its
# hardware encoder probes are not loaded. Needs core video, so it runs
# (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
