# The task queue entry points (obs_in_task_thread, obs_queue_task and
# obs_wait_for_destroy_queue) on a live core with the graphics, audio and
# destroy threads running (#5560). Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display. Registered once in
# build/test-graph.json, consumed by CMake and Cargo.
