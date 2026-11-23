# Stopping a Custom FFmpeg Output freed its encoders while the video or
# audio thread could still be encoding with them
# (obsproject/obs-studio#7536). Builds the real obs-ffmpeg-output.c into the
# test and wraps two libavcodec calls with the GNU linker's --wrap. Needs
# core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
