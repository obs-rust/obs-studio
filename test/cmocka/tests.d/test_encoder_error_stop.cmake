# Encoder error stops outputs without deadlocking or breaking idle outputs
# (obsproject/obs-studio#9946). Builds obs-ffmpeg-mux.c directly for the real
# replay_buffer output. Needs core video, so it runs (under xvfb-run) on Linux
# and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
