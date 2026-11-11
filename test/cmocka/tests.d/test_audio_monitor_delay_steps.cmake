# Characterization of the WASAPI monitor's delay decisions of
# libobs/audio-monitoring/win32/wasapi-output.c (process_audio_delay()), moved
# to libobs/audio-monitoring/monitor-delay.c and ported to
# rust/obs-audio-monitoring (monitor_delay): the stale cut and the front
# packet's verdict (hold, drag or play) with its 75 ms, 5 s and video stall
# thresholds. A stand-in puts the delay loop together in the C's order. Builds
# the moved file on its own: no Windows header and no libobs core, so it runs
# on every platform. The functions are hidden in libobs, so this characterizes
# that C source; the test calls only them. The Tier 3 parity tests in
# rust/obs-audio-monitoring compare the shims with this C too.
# Target and test are declared in ../build/test-graph.d.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). audio_monitor_delay_* are
# hidden in the shared library, so the test takes them from the production
# staticlib at executable link time and compiles none of the C. It includes
# only monitor-delay.h and monitor-video-sync.h, and the
# obs-audio-monitoring member needs nothing from libobs, so it links the
# archive alone. Linux only: the shims have no host code, so one host shows
# the swap.
# Rust-static target and test are declared in ../build/test-graph.d.
