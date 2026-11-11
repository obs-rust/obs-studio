# Scene group transform (a point through a matrix, and the scale/rotation
# fold). Builds libobs/obs-scene-group-transform.c.h on its own: no display
# and no libobs core, so it runs on every platform. The symbols are not
# exported, so this characterizes that C source.
# Defined in build/test-graph.d and registered by the parent CMakeLists.txt.
