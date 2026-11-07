# Hiding a source that is only in the Studio Mode Preview must hide it
# (obsproject/obs-studio#4120). Hiding an item with audio is applied as an audio
# action, which needs core video and audio, so it runs (under xvfb-run) on Linux
# only.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
