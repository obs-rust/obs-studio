# Hiding a group must stop a child whose scene item has show/hide transitions
# (obsproject/obs-studio#5551). Hiding a group is applied by the audio thread,
# which needs core video, so it runs (under xvfb-run) on Linux only.
# Defined in ../build/test-graph.d.
