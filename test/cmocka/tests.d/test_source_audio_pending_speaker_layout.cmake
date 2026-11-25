# obs_source_audio_pending and obs_source_get_speaker_layout (#5603): NULL
# answers true / SPEAKERS_UNKNOWN and logs the validity check's name once
# (obs_source_get_audio_channels for the layout, upstream's quirk), a removed
# source or one with neither OBS_SOURCE_AUDIO nor OBS_SOURCE_COMPOSITE answers
# pending, otherwise audio_pending; the layout is sample_info.speakers, and
# nothing is written. Test-owned struct obs_source, no core. Registered once
# in build/test-graph.json, consumed by CMake and Cargo.
