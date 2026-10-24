# Characterization of the source load migration and value rules of
# obs_load_source_type() in libobs/obs.c, moved to libobs/obs-source-load.c
# before their port to rust/obs-core (source_load): the saved prev_ver as a
# uint32_t (an int64 past UINT32_MAX truncates, characterized, not endorsed),
# the monitoring migration at, just below and just above 23.2.2 and 33.0.0 for
# each monitoring type and values outside the enum, with and without
# OBS_SOURCE_MONITOR_BY_DEFAULT, and the volume (#2128), balance and delay
# (#2058) and deinterlace mode (#2120) rules at their edges, NaN, +-inf and -0
# included. Builds the moved file on its own: no display, so it runs on every
# platform. The functions are hidden in libobs, so this characterizes that C
# source; the test calls only them. test_source_audio_ranges,
# test_source_volume_mix and test_deinterlace_mode_load cover the caller
# through the real libobs.
# Target and test are declared in ../build/test-graph.json.
