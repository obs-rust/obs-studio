# The reconnect route of obs_output_signal_stop() through the libobs that
# ships, which test_output_reconnect_steps (it compiles
# obs-output-reconnect.c.h into itself) does not reach: a registered raw
# output reports stops as a plugin does. It pins the reconnect callback being
# asked and refusing, not being asked with a retry count of 0, an error or a
# clean stop stopping the output, and a stop while the reconnect thread waits
# ending the reconnect before any attempt. obs_startup needs an X display on
# Linux. Defined in ../build/test-graph.json.
