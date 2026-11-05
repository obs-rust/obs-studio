# The immediate-mode vertex builder of graphics.c through libobs's exports
# (#3132): a draw in each mode, the immediate cap, and the stop and save with
# no vertex, which leave no vertex buffer data behind and free every array. A
# fake device in the test owns the vertex buffers; no GPU, no display. ASan
# and LeakSanitizer check libobs's allocations.
# Defined by the canonical native test graph (build/test-graph.json).
