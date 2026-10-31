# Characterization of the WASAPI monitor's speaker layout and video stall
# tracker, moved from the static inline bodies of monitor-speaker-layout.h and
# monitor-video-sync.h to libobs/audio-monitoring/monitor-speaker-layout.c and
# monitor-video-sync.c: every mask with 0 to 9 channels, 7 channels and counts
# past 8, and the tracker fed a 0 timestamp, the same one, earlier ones and a
# count that wraps. Builds the moved files on their own: no Windows header and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# Target and test are declared in ../build/test-graph.json.
