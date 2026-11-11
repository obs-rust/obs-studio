# A PipeWire camera unplugged while the camera portal lists it
# (obsproject/obs-studio#13698): the camera portal against a fake portal on a
# private D-Bus bus and a camera node on a private PipeWire daemon that
# test_pipewire_cursor_hdr.py starts. Needs an X display (xvfb-run) for
# obs_startup(), dbus-daemon and pipewire; skips (77, or cmocka skips) without
# them. Linked with --wrap to see the camera portal's device ids being freed.
# Shared unchanged fixture registration: build/test-graph.d (Refs #1390).
