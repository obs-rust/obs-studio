# Characterization of the effect parser's function parser and its lookups
# (#3623): ep_parse() on an effect's text, then the functions it kept (return
# type, name, mapping, param vars, contents and the struct, func, sampler and
# param dependencies), the parse errors and the allocations still held after
# ep_free(), written out and compared whole, for well-formed and malformed
# signatures, bodies, dependencies, specifier keywords and text cut short at
# each step of a signature. ep_parse() is hidden in libobs, so the test
# compiles the original effect parser C into itself. No display and no
# graphics context, so it runs on every platform. Target and test are declared
# in ../build/test-graph.json.
