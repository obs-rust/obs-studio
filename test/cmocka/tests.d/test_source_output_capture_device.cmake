# obs_source_audio_output_capture_device_changed and its activate handler
# (#5824), called directly: a source without OBS_SOURCE_DO_NOT_SELF_MONITOR
# is ignored; a match sends deduplication_changed with the source, logs one
# line and connects the activate handler; a non-match stops only the
# deduplicating source. Interposes external libpulse symbols in the
# executable, as test_set_audio_monitoring_device does. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
