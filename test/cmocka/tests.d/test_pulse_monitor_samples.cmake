# End to end for the samples PulseAudio audio monitoring plays: libobs
# monitoring into null sinks in s16le, s32le, u8 and float32le at source
# volumes 0.5, 1 and 2 (saturation, #1709), recorded from each sink's monitor
# with parec, against a private PipeWire sound server started by
# test_pulse_monitor_device.py. Skipped (exit 77) when PipeWire, WirePlumber,
# pipewire-pulse or parec are not installed.
# Defined in build/test-graph.json and registered by the parent CMakeLists.txt.
