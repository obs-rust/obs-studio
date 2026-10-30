# Characterization of the reconnect policy of libobs/obs-output.c, moved to
# libobs/obs-output-reconnect.c before its port to rust/obs-output
# (output_reconnect, #2990): the first attempt's wait from the delay setting,
# the waits growing by the exponent to the 15-minute limit (the default 2 s at
# 1.5 and 1.55, other settings and exponents), a delay of 0, the give-up after
# the retry count, whether reconnecting is on, and the route of a stop (an
# active output reconnects only on a disconnect, a failed attempt retries on
# any error). A delay of 0 is characterized, not endorsed. The inputs the port
# changes on purpose (a delay setting that overflows or is negative, a first
# delay above the limit, a later product above UINT32_MAX, a negative count)
# are not covered. Builds the moved file on its own: no display and no libobs
# core, so it runs on every platform. The functions are hidden in libobs, so
# this characterizes that C source; the test calls only them. The Tier 3
# parity tests in rust/obs-output compare the shims with this C.
# Target and test are declared in ../build/test-graph.json.
