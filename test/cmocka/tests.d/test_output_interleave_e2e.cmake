# The output interleaver end to end through the libobs that ships, which
# test_output_interleave (it compiles obs-output-interleave.c.h into itself)
# does not reach: registered video and two audio encoders, an encoded output
# and media fed by hand, as in test_output_interleave_restart. It uses real
# core video, so it needs an X display on Linux (xvfb-run, as in CI).
# Defined in ../build/test-graph.d.
