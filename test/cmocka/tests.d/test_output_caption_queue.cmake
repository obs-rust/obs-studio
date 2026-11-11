# Caption queue lifecycle: the stop path must drain caption_text under
# caption_mutex, and destroying a track must free queued nodes.
# Reads the caption queue through obs-internal.h, so it needs the private
# libobs and libcaption include dirs.
# Defined in ../build/test-graph.d.
