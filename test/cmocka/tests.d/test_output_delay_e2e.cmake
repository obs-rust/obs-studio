# The output stream delay end to end through the libobs that ships, which
# test_output_delay (it compiles obs-output-delay.c.h into itself) does not
# reach: a registered audio encoder and encoded output, a real-time audio
# thread, and one-second delays, so the cases take about 13 s in all.
# obs_startup needs an X display on Linux.
# Defined in ../build/test-graph.d.
