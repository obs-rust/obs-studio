# The module-list seam (libobs/obs-module-lists-seam.h), which the port of
# the module list calls (libobs/obs-module-lists.c) calls: each read against
# the field it wraps, and each darray address against the typed list behind
# it and the obs_module_add_* calls that push onto a module's lists. The
# accessors are hidden in libobs, so the test builds the seam into itself,
# over a stand-in core and stand-in modules.
# Target and test are declared in ../build/test-graph.json.
