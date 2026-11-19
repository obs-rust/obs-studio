# Registered in the canonical native test graph (test/cmocka/build/test-graph.json).
# test_audio_capture_callbacks_rust runs the unchanged test_source_audio_capture_callbacks.c
# against libobs_rust-static. It sits outside the test_source_ prefix so the source-step
# fragments do not give it their aborting blog stub: it links the real variadic blog
# (base-variadic.c.h) over the Rust log handler and checks the real diagnostics.
