# A PipeWire camera sending NV12 (obsproject/obs-studio#13823) or only I420 or
# YV12 (obsproject/obs-studio#13744): the camera source's stream code
# (pipewire.c) against a producer stream on a private PipeWire daemon that
# test_pipewire_cursor_hdr.py starts, with the output read back. Needs an X display (xvfb-run) for obs_startup() and pipewire; skips
# (77, or cmocka skips) without them. The no-display half is
# test_pipewire_frame_layout.
# Shared unchanged fixture registration: build/test-graph.d (Refs #1390).
