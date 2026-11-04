# Characterization of the effect parser's parameter value assignments
# (#3610): ep_parse() on an effect's text, then the params it kept (flags,
# array count and every byte of the default value), the functions after them,
# the parse errors and the allocations still held after ep_free(), written out
# and compared whole, for each value form, malformed values and text cut short
# at each step. ep_parse() is hidden in libobs, so the test compiles the
# original effect parser C into itself. No display and no graphics context, so
# it runs on every platform. Target and test are declared in
# ../build/test-graph.json.
