# Characterization of the monitoring, monitor-only mute, balance, mixer,
# flag, push-to and audio active rules of the sources in
# libobs/obs-source.c, moved to libobs/obs-source-monitoring.c and ported to
# rust/obs-source (monitoring): the same-value rule, the mixers' audio gate
# and stored value, the push-to log rule, the deprecated and stored
# monitoring types, the monitor route, the steps of
# obs_source_set_monitoring_type() and the audio active signal over every
# combination of their inputs, and the balance clamp over the edges of
# float; and copies of the callers on a stand-in source that records its
# signals, logs, mute calls and monitor: every monitoring type from every
# start state, the compatibility signal's type, the balance and mixers a
# handler leaves, the push-to enables and the audio active signal. Builds
# the moved file into itself. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them.
# test_source_monitoring_type, test_source_monitor_only_load and
# test_source_audio_balance_mono cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.d.

# Exercise the unchanged characterization against the Rust shims. The test
# calls only the thirteen hidden monitoring functions, and includes their
# header and obs.h (for the libobs values the header mirrors; obs.h pulls SIMDe
# and obsconfig.h, hence libobs's include directories), so it links the archive
# alone: its obs-source member needs nothing from libobs or the other crates.
# libm is for nextafterf(), as the native graph links it off MSVC. Linking the
# archive directly avoids exporting private symbols just for a test.
# Rust-static target and test are declared in ../build/test-graph.d.
