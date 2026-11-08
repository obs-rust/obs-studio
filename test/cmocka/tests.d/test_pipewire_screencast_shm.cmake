# PipeWire screen capture of frames in shared memory must follow the chunk's
# stride, offset and size (obsproject/obs-studio#4961, distorted capture;
# obsproject/obs-studio#13318, crash in memmove): the screencast portal source
# against a fake portal on a private D-Bus bus and a producer stream on a
# private PipeWire daemon that test_pipewire_cursor_hdr.py starts. Needs an X
# display (xvfb-run) for obs_startup(), dbus-daemon and pipewire; skips (77,
# or cmocka skips) without them.
# Shared unchanged fixture registration: build/test-graph.json (Refs #1390).
