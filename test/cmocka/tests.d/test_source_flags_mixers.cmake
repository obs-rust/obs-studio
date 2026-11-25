# obs_source_set_flags and obs_source_set_audio_mixers (#5610): NULL logs the
# validity check's line once; set_flags stores a change, then emits
# update_flags; set_audio_mixers leaves a source without audio (that does not
# own its info id) alone, and on a change emits audio_mixers before the store,
# storing what the handlers leave in the calldata, truncated to 32 bits.
# Test-owned struct obs_source values with a real signal handler, no core.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
