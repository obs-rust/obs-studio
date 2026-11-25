# The obs_source_media_* control calls (#5616): NULL logs the validity
# check's line once; play_pause, restart, stop, next, previous and set_time
# queue exactly one media_action under media_actions_mutex for a source with
# context data, OBS_SOURCE_CONTROLLABLE_MEDIA and their own callback, and
# leave the queue and the mutex alone otherwise; started and ended emit their
# signal with only the source, on the source's own handler only, for any
# controllable source. Test-owned struct obs_source values with a real signal
# handler and queue, under a started core.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
