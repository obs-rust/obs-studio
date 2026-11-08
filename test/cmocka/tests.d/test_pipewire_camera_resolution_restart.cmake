# A new resolution for a streaming PipeWire camera was renegotiated on the
# running stream (obs-rust/obs-studio#390, obsproject/obs-studio#10399;
# libcamera cameras then drop the stream): the camera portal against a fake
# portal on a private D-Bus bus and a camera node that, like libcamera,
# refuses a new format while it streams, on a private PipeWire daemon that
# test_pipewire_cursor_hdr.py starts. Needs an X display (xvfb-run) for
# obs_startup(), dbus-daemon and pipewire; skips (77, or cmocka skips) without
# them.
# Shared unchanged fixture registration: build/test-graph.json (Refs #1390).
