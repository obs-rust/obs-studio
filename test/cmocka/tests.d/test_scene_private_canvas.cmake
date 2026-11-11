# Private scenes and Studio Mode's private scene duplicates get the canvas of
# the scene they copy (upstream obsproject/obs-studio PR #12824). Needs core
# video for a second canvas, so it runs (under xvfb-run) on Linux only.
# libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.d.
