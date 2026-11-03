# Characterization of the shader parser's sampler-state and struct blocks
# through the real libobs (#3526): shader_parse() on a shader's text, then the
# samplers, structs and params it kept and the parse errors, written out and
# compared whole, for well-formed blocks, malformed ones and text cut short
# inside each block. No display and no graphics context, so it runs on every
# platform. Target and test are declared in ../build/test-graph.json.
