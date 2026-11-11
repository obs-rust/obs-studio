# Start Recording on a path that cannot be written failed, and the next Start
# Recording crashed OBS (obsproject/obs-studio#11940,
# obsproject/obs-websocket#1216): a failed ffmpeg_muxer start released the
# output's settings twice. Builds obs-ffmpeg-mux.c and runs the real output
# against core video, so it runs under xvfb-run on Linux and skips without a
# display. obs_data_release() is wrapped to count the releases.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
