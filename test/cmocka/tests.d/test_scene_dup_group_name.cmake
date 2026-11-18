# Duplicating a scene names the copy of a group after the group, not
# __unnamedNNNN (obsproject/obs-studio#14145, #4991). Each case forks a child
# with core video, so it runs (under xvfb-run) on Linux and skips without a
# display. libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.json.
