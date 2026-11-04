# Characterization of the effect parser's passes and techniques (#3598):
# ep_parse() on an effect's text, then the techniques, their passes and the
# vertex and pixel program tokens it kept, the params and functions after
# them, the parse errors and the allocations still held after ep_free(),
# written out and compared whole, for well-formed techniques, malformed ones
# and text cut short at each step. ep_parse() is hidden in libobs, so the test
# compiles the original effect parser C into itself. No display and no
# graphics context, so it runs on every platform. Target and test are
# declared in ../build/test-graph.json.
