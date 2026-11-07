# gs_draw_sprite() and a full-size gs_draw_sprite_subregion() must honour
# GS_FLIP_U and GS_FLIP_V each on its own axis (obs-rust/obs-studio#2124:
# GS_FLIP_U drew a 2D texture upside down). Needs core video, so it runs
# (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
