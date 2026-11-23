# Characterization of obs_set_audio_monitoring_device and
# obs_reset_audio_monitoring before their Rust swap (#5489): the stored copy,
# the per-monitor reset, the same-id no-op and the Audio Output Capture
# notification. Interposes external libpulse symbols in the executable, as
# test_pulse_monitor_reset does. Registered once in build/test-graph.json.
