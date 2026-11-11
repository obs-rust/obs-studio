# Camera controls of the PipeWire camera source (obs-rust/obs-studio#827):
# sliders could not be dragged (every control change asked the Properties
# window to rebuild), manual controls stayed enabled while their automatic
# mode was on, and menu controls were missing. The camera portal against a
# fake portal on a private D-Bus bus and two V4L2 camera nodes on a private
# PipeWire daemon that test_pipewire_cursor_hdr.py starts. Needs an X display
# (xvfb-run) for obs_startup(), dbus-daemon and pipewire; skips (77, or cmocka
# skips) without them.
# Shared unchanged fixture registration: build/test-graph.d (Refs #1390).
