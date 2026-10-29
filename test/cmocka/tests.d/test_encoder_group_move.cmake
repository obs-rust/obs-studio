# obs_encoder_set_group() must check the new group before it leaves the old
# one (obs-rust/obs-studio#2932): a move into a group with started encoders
# fails and leaves the encoder in its old group. The test marks a group
# started through obs-internal.h, so it needs no video; obs_startup needs an
# X display on Linux (run under xvfb-run when it is installed).
# Defined in ../build/test-graph.json.
