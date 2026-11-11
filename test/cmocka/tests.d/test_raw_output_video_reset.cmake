# Raw outputs follow obs_reset_video() instead of keeping the freed video_t
# (obsproject/obs-websocket#1344, exeldro/obs-source-record#143,
# DistroAV/DistroAV#1096). Needs core video, so it runs (under xvfb-run) on
# Linux and skips without a display.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
