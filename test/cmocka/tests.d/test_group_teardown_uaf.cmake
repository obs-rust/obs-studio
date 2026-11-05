# obs_shutdown must wait for the whole chain of queued scene/group/source
# destruction (use after free that crashed Windows). Needs no graphics, but
# obs_startup needs an X display on Linux, so run under xvfb-run when it
# exists; the case skips if obs_startup still fails.
# test_group_teardown_uaf is defined in build/test-graph.json.
