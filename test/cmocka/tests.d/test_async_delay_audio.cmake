# A Video Delay (Async) filter of more than 2 s must not break up the audio of
# the source it delays, and the audio must stay delayed with the video
# (upstream obsproject/obs-studio#13830). Builds async-delay-filter.c directly
# so the obs-filters module is not loaded. Mixing needs core video, so it runs
# (under xvfb-run) on Linux and skips without a display.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
