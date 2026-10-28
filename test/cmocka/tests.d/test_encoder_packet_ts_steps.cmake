# Characterization of the packet timestamp decisions of libobs/obs-encoder.c,
# moved to libobs/obs-encoder-packet-ts.c before their port to rust/obs-output
# (encoder_packet_ts, #2798): the packet timebase of do_encode() and the GPU
# encode thread, packet_dts_usec() (common rates, a negative DTS from B-frames,
# truncation toward 0), the packet's dts_usec from the encoder's start time and
# the first packet's offset, sys_dts_usec after the pause offset, and the
# newest timing entry of a PTS. The unsigned wraps and the int32_t and int64_t
# stores at the limits are characterized, not endorsed; the inputs where C is
# undefined are not covered. Builds the moved file on its own: no display and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_encoder_error_stop, test_encoder_join_keyframe,
# test_multitrack_encoder_override and test_media_packet_eof_leak cover the
# callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
