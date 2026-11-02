# Characterization of the shader parser's number reads through the real libobs
# (#3468): shader_parse() on a shader's text, then each parameter's array
# count and default value bytes, and the parse errors, with the row and
# column errors of a bad floatNxM type. Only values inside int range, so it
# runs unchanged against both libobs builds; no display and no graphics
# context, so it runs on every platform. Target and test are declared in
# ../build/test-graph.json.
