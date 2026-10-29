# Characterization of the encoder group reconfigure rules of
# libobs/obs-encoder.c, moved to libobs/obs-encoder-group-reconfig.c before
# their port to rust/obs-output (encoder_group_reconfig, #2913): the request
# of obs_encoder_update() for an active grouped encoder (the first, one under
# way, one after this encoder applied it, one after all did), the request
# counter, the frame step of handle_encoder_group_reconfigure_request() (the
# next shared frame rounded up to timebase_num * LCM, the largest wins, each
# apply condition, the held request on the last apply), the group over frames
# with divisors 1, 2 and 3, gcd() and lcm() of add_connection(), and the reset
# of remove_connection(). The uint32_t wraps and the counter wrap are
# characterized, not endorsed; the inputs where C is undefined are not
# covered. Builds the moved file on its own: no display and no libobs core, so
# it runs on every platform. The functions are hidden in libobs, so this
# characterizes that C source; the test calls only them. There is no _rust
# variant against libobs_rust-static: the shims' archive member also holds the
# rest of obs-output, which needs libobs's hidden output and service seams and
# obs-util, so it does not link on its own, as for
# test_encoder_lifecycle_steps. The Tier 3 parity tests in rust/obs-output
# compare the shims with this C instead. test_encoder_group_reconfig_walk
# covers the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
