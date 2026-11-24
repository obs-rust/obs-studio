# obs_source_get_audio_timestamp / obs_source_get_audio_mix (#5589): NULL
# logs the caller's name, the timestamp is audio_ts alone read under
# audio_buf_mutex, and the mix getter copies every audio_output_buf pointer.
# Test-owned struct obs_source, no core. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
