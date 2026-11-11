# Show / Hide Transition from the Sources context menu reaches one of the
# selected sources only (obsproject/obs-studio#10448). Builds the plain C
# helper the frontend calls, so the test does not link the Qt frontend.
# Transitions need graphics, so it loads the built OpenGL module and runs (under
# xvfb-run) on Linux only; without a display the cases skip.
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
