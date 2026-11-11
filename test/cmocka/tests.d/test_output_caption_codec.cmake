# Captions queued on a video track whose codec cannot carry them (not h264,
# hevc or av1) must not leak the packet copy add_caption() makes. Drives a
# real output through the libobs that ships, as test_output_interleave_restart
# does, and reads the caption queue through obs-internal.h. It uses real core
# video, so it needs an X display on Linux (xvfb-run, as in CI).
# Defined in ../build/test-graph.d.
