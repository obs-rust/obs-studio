# Studio Mode with "Duplicate Sources" must copy the sources inside groups too
# (obsproject/obs-studio#4309, #6977). Each case forks a child with core video,
# so it runs (under xvfb-run) on Linux and skips without a display.
# libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.d.
