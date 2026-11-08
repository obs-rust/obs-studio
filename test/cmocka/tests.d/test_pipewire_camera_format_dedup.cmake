# A libcamera camera lists its default size twice in its EnumFormat params, so
# the camera source's format list showed it twice (obs-rust/obs-studio#1068):
# the camera portal against a fake portal on a private D-Bus bus and a camera
# node on a private PipeWire daemon that test_pipewire_cursor_hdr.py starts.
# Needs an X display (xvfb-run) for obs_startup(), dbus-daemon and pipewire;
# skips (77, or cmocka skips) without them.
# Shared unchanged fixture registration: build/test-graph.json (Refs #1390).
