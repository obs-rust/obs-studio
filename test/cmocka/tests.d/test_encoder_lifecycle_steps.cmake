# Characterization of the encoder callback and connection lifecycle decisions
# of libobs/obs-encoder.c, moved to libobs/obs-encoder-lifecycle.c and ported
# to rust/obs-output (encoder_lifecycle): the first and last callback, the
# callback found, the connection route and the texture cap, the encoder group
# counters and the SEI in front of the first keyframe, with the type tests the
# route, the keyframe request and the first packet route share with the setters
# (libobs/obs-encoder-bind.c and libobs/obs-encoder-audio-start.c, which the
# test also builds). The bodies of
# obs_encoder_start_internal(), obs_encoder_stop(), add_connection(),
# remove_connection(), gpu_encode_available(), full_stop(), send_packet() and
# send_first_video_packet() are also copied onto a stand-in encoder, so the
# start with 0, 1 and 2 callbacks and a duplicate, the stop of a missing, a
# non-last and the last callback, every connect route, the group counters and
# destroy, the full stop of idle outputs and of a texture encoder and the first
# packets with each SEI answer are pinned end to end (the count wrap, the LCM of
# a 0 divisor, the route asked again at the stop and the packet times cleared
# for a missing callback: characterized, not endorsed). Builds the three files on
# their own: no display, no GPU and no thread, so it runs on every platform.
# The functions are hidden in libobs, so this characterizes that C source; the
# test calls only them. There is no _rust variant against libobs_rust-static:
# the shims' archive member also holds the rest of obs-output, which needs
# libobs's hidden output and service seams and obs-util, so it does not link on
# its own, as for test_encoder_audio_start_steps. The Tier 3 parity tests in
# rust/obs-output compare the shims with this C instead.
# test_encoder_join_keyframe, test_encoder_error_stop,
# test_encoder_group_null and test_multitrack_ladder cover the callers through
# the real libobs.
# Target and test are declared in ../build/test-graph.json.
