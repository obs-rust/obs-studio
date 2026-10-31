# Characterization of the decisions of the immediate-mode vertex builder of
# libobs/graphics/graphics.c, moved to libobs/graphics/graphics-immediate.c
# before their port to rust/obs-graphics (immediate, #3132): the push gates
# (the cap at GS_IMMEDIATE_COUNT in immediate mode, the texture units), the
# draw count of gs_render_stop() (each shorter normal, colour or texture 0
# array cuts it, and each differing one logs) and the buffer of
# gs_render_save() (its vertex count and texture arrays). A longer array that
# logs and an empty texture array 0 before a filled one are characterized, not
# endorsed. The inputs the port changes on purpose (a texture unit outside
# 0-15 or above 0 in immediate mode, a save with a shorter array, the arrays a
# save leaves) are not covered. Builds the moved file on its own: no display
# and no graphics context, so it runs on every platform. The functions are
# hidden in libobs, so this characterizes that C source; the test calls only
# them. The rendering tests cover the callers through the real libobs.
# Target and test are declared in ../build/test-graph.json.
