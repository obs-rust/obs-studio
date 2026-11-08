# Characterization of the scene item visibility audio ramp in
# libobs/obs-scene.c before its port: the sample of the program mix on which
# a show or hide lands, and the active references the applied hides drop.
# Public API plus the item and source refcounts, so it also covers the hybrid
# libobs. Applying the actions needs core video and audio, so it runs (under
# xvfb-run) on Linux only.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
