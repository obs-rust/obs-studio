# obs_scene_insert_group() with a non-NULL item list and a count of 0 must add
# an empty group on top, not read before the list (obs-rust/obs-studio#2945).
# Each case forks a child with its own libobs and needs no video;
# obs_startup() needs a display on Linux, so the test runs under xvfb-run when
# it is installed and the cases skip without a display.
# test_group_insert_empty is defined in build/test-graph.json.
