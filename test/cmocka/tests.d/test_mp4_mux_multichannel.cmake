# Hybrid MP4 audio sample entries for more than two channels (OBS forum
# thread 196049: 5.1 recordings silent in VLC 3). Builds obs-outputs'
# mp4-mux.c into the test and muxes stub encoder packets into memory;
# obs_startup needs a display, so it runs (under xvfb-run) on Linux and
# skips without one.
# Defined in ../build/test-graph.d.
