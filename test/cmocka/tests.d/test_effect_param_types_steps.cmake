# Characterization of the parameter type decisions of the effect parser,
# libobs/graphics/effect-parser.c, moved to
# libobs/graphics/effect-param-types.c before their port to rust/obs-graphics
# (effect_param_types, #3162): the shader parameter type of a type name, the
# default-value parser a parameter gets, and the value count of an int or
# float array type with its row and column errors. A name that takes the
# string or array parser but has an unknown type, and a bad digit that still
# counts, are characterized, not endorsed. Builds the moved file on its own: no
# display and no graphics context, so it runs on every platform. The functions
# are hidden in libobs, so this characterizes that C source; the test calls
# only them. The effect tests cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
