# Characterization of the WASAPI monitor's speaker layout and video stall
# tracker, moved from the static inline bodies of monitor-speaker-layout.h and
# monitor-video-sync.h to libobs/audio-monitoring/monitor-speaker-layout.c and
# monitor-video-sync.c: every mask with 0 to 9 channels, 7 channels and counts
# past 8, and the tracker fed a 0 timestamp, the same one, earlier ones and a
# count that wraps. Builds the moved files on their own: no Windows header and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# Target and test are declared in ../build/test-graph.d.

# The same cases against the Rust shims, built and run only with the hybrid
# backend (OBS_LIBOBS_BACKEND=C has no Rust target). audio_monitor_speaker_layout
# and audio_monitor_track_video_ts are hidden in the shared library, so the
# test takes them from the production staticlib at executable link time and
# compiles none of the C. The obs-audio-monitoring member needs nothing from
# libobs, so it links the archive alone. Linux only: the shims have no host
# code, so one host shows the swap.
# Rust-static target and test are declared in ../build/test-graph.d.
