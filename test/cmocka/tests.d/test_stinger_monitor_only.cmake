# A Stinger transition can be set to Monitor Only again (upstream
# obsproject/obs-studio#14144): with monitoring and "monitor_only" on, its
# clip is monitored and muted. Loads the built obs-transitions module
# (HAVE_TRANSITIONS_TEST_OWNER, set by test_stinger_media_duration.cmake).
# Needs core video, so it runs (under xvfb-run) on Linux.
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
