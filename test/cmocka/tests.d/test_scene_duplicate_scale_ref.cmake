# Duplicated scenes (Studio Mode program, "Duplicate Scene", ungroup) must draw
# items at the same size as the original (obsproject/obs-studio#12171). Renders
# scenes, so this needs core video and runs (under xvfb-run) on Linux only.
# libobs finds its effect files relative to the working directory.
# Defined in ../build/test-graph.d.
