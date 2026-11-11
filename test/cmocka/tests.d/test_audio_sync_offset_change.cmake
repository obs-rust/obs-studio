# A source's audio sync offset must apply while it is producing audio,
# including a change away from 0 and a saved offset set on load
# (upstream obsproject/obs-studio#7912). Mixing needs core video, so it runs
# (under xvfb-run) on Linux and skips without a display.
# libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.d.
