# Characterization of the pause checks of libobs/obs-encoder.c, moved to
# libobs/obs-encoder-pause.c before their port to rust/obs-output
# (encoder_pause, #3033): a video frame before, at the start of, inside and
# at the end of a pause; an audio buffer before the pause (passed), holding
# its start (cut there), wholly inside (dropped) and holding its end (cut
# there and stamped with the pause start before the offset is taken off); a
# start or an end exactly at either end of a buffer, the truncating
# conversions, and the offset taken off every buffer. The stamp, a frame
# stamped 0 and the unsigned wraps are characterized, not endorsed. The
# inputs the port changes on purpose (a video frame after a set pause end
# that never lands on it, a paused buffer that starts after a set pause end)
# are not covered. Builds the moved file on its own: no display and no libobs
# core, so it runs on every platform. The functions are hidden in libobs, so
# this characterizes that C source; the test calls only them.
# test_replay_buffer_pause_stop covers the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
