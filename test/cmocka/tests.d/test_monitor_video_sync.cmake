# The audio monitor's video sync check after a Media Source with embedded
# cover art restarts (obsproject/obs-studio#9428, #11739). Shared code, so
# it runs on every platform.
# Defined in ../build/test-graph.json.

# The same cases against the Rust shim, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). audio_monitor_track_video_ts
# is hidden in the shared library, so the test takes it from the production
# staticlib at executable link time and compiles none of the C. The
# obs-audio-monitoring member needs nothing from libobs, so it links the
# archive alone. Linux only: the shim has no host code, so one host shows the
# swap.
# Rust-static target and test are declared in ../build/test-graph.json.
