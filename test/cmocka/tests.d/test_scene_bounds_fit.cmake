# A source with no size must get a finite bounding box scale
# (obsproject/obs-studio#4245). Builds libobs/obs-scene-bounds.c.h on its own:
# no display and no libobs core, so it runs on every platform.
# Defined in ../build/test-graph.d.
