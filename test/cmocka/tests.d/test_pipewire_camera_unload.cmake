# The camera portal unloading (OBS quitting) while its PipeWire thread runs:
# the camera portal against a fake portal on a private D-Bus bus and a camera
# node on a private PipeWire daemon that test_pipewire_cursor_hdr.py starts.
# Needs an X display (xvfb-run) for obs_startup(), dbus-daemon and pipewire;
# skips (77, or cmocka skips) without them. Linked with --wrap to check that
# PipeWire proxies are destroyed on the loop thread or under the loop lock.
# Shared unchanged fixture registration: build/test-graph.json (Refs #1390).
