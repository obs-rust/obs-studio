# Scale filtering must keep working on rotated scene items
# (obsproject/obs-studio#12625). Renders scenes, so this needs core video and
# runs (under xvfb-run) on Linux only; the cases skip without a display.
# libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.d.
