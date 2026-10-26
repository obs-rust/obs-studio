# Characterization of the PulseAudio monitor's open and write steps of
# libobs/audio-monitoring/pulse/pulseaudio-output.c, moved to
# pulseaudio-monitor-steps.c before their Rust port: do_stream_write()'s grow,
# cork and write rules (the uint32_t tlength * 2 and the truncated new tlength,
# characterized, not endorsed), on_audio_playback()'s skip, byte count and
# volume rules, pulseaudio_sink_info()'s results, audio_monitor_init()'s
# device, feedback, fallback, format, 25 ms buffer and flags, on_sink_event()'s
# filter, audio_monitor_reset()'s context rule and audio_monitor_destroy()'s
# last monitor. Stand-ins put the write loop, the open routes and the reset
# routes together in the C's order. Builds the moved file and
# pulseaudio-stream-flags.c (for pulseaudio_monitor_stream_flags()) into
# itself; no server. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them. test_pulse_monitor_device,
# test_pulse_default_follow, test_pulse_monitor_reset,
# test_pulse_stream_restart, test_pulse_two_sources and
# test_pulse_monitor_samples cover the callers through the real libobs against
# a private sound server.
# Target and test are declared in ../build/test-graph.json.
