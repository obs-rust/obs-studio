# Characterization of the effect parser's struct and sampler-state blocks
# (#3550): ep_parse() on an effect's text, then the structs, samplers, params
# and functions it kept, the parse errors and the allocations still held after
# ep_free(), written out and compared whole, for well-formed blocks, malformed
# ones and text cut short inside each block. ep_parse() is hidden in libobs, so
# the test compiles the original effect parser C into itself. No display and
# no graphics context, so it runs on every platform. Target and test are
# declared in ../build/test-graph.json.
