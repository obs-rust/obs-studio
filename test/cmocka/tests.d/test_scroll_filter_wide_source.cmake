# A Scroll filter must show a source wider or taller than the largest GPU
# texture (OBS forum "long text file scrolling. Text disappears";
# obsproject/obs-studio#10034). Loads the built obs-filters module. Needs core
# video, so it runs (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
