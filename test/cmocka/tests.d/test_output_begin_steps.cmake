# Characterization of the begin-capture plan of libobs/obs-output.c, moved to
# libobs/obs-output-begin.c before its port to rust/obs-output (output_begin,
# #2927): each encoder's packet interval of calculate_batch_size() (video at
# 24, 25, 29.97, 30, 59.94 and 60 fps with divisors 1 to 3; AAC at 44.1, 48
# and 96 kHz, Opus at 48 kHz), the interleaver's maximum batch size from those
# intervals (one encoder, video and audio, a multitrack set, equal intervals,
# none) and the two gates of pair_encoders(). The uint32_t wrap of
# fps_den * divisor and the wrap of largest * 2 are characterized, not
# endorsed; the inputs where C is undefined (a 0 fps_num, sample rate or
# interval) are not covered. Builds the moved file on its own: no display and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# There is no _rust variant against libobs_rust-static: the shims' archive
# member also holds the rest of obs-output, which needs libobs's hidden output
# and service seams and obs-util, so it does not link on its own, as for
# test_encoder_lifecycle_steps. The Tier 3 parity tests in rust/obs-output
# compare the shims with this C instead. test_output_start_steps,
# test_output_interleave, test_output_interleave_e2e and
# test_output_interleave_restart cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
