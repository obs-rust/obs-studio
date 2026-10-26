# The deprecated obs_source_set_monitoring_type() and
# obs_source_get_monitoring_type() keep their three types on 33.0 (#2447):
# Monitor Only is monitoring on and muted and stays out of the program mix,
# Monitor and Output is on and unmuted, None is off and undoes only a mute
# Monitor Only set. Mixing needs core video, so it runs (under xvfb-run) on
# Linux and skips without a display. libobs finds its effect files relative
# to the working directory.
# Defined in ../build/test-graph.json.
