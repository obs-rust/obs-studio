# A Color Correction filter at neutral settings must leave a high-precision
# (GS_CS_SRGB_16F) source as it is, not turn its out-of-gamut colours black
# (upstream obsproject/obs-studio#10403). Loads the built obs-filters module.
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
