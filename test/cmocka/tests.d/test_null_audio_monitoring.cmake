# Target and direct test command live in ../build/test-graph.json.
# Only Linux/FreeBSD with ENABLE_PULSEAUDIO=OFF link the real null backend.
# Public assertions exercise that library; renamed private assertions exercise
# the unchanged C source only, to be mirrored against Rust in the later swap.
