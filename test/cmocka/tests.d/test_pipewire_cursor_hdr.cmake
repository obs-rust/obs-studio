# The PipeWire capture cursor in Rec. 2100 (PQ/HLG) output
# (obsproject/obs-studio#13350): the screencast portal source against a fake
# portal on a private D-Bus bus and a producer stream with cursor metadata on
# a private PipeWire daemon that test_pipewire_cursor_hdr.py starts. Needs an
# X display (xvfb-run) for obs_startup(), dbus-daemon and pipewire; skips
# (77, or cmocka skips) without them.
# Shared unchanged fixture registration: build/test-graph.json (Refs #1390).
