# Characterization of the frame timing decisions of the graphics thread loop in
# libobs/obs-video.c, moved to libobs/obs-video-frame-timing.c before their port
# to rust/obs-core (video_frame_timing, #2754): tick_sources()'s time since the
# last tick (one interval on the first frame) and its float seconds, a clock
# that goes backwards, and the frame statistics at the end of
# obs_graphics_thread_loop() (the sums, the one-second window, fps, average
# render time, reset, wraps at the limits). Values outside what libobs sets are
# characterized, not endorsed. Builds the moved file on its own: no display and
# no libobs core, so it runs on every platform. The functions are hidden in
# libobs, so this characterizes that C source; the test calls only them.
# test_source_video_tick_steps, test_source_profiler,
# test_media_restart_stale_frame, test_transition_recursive_destination,
# test_slideshow_empty and test_slideshow_manual_end cover the callers through
# the real libobs.
# Target and test are declared in ../build/test-graph.json.
