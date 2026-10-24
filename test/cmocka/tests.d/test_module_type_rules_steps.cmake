# Characterization of the output, encoder and service registration rules of
# libobs/obs-module.c, moved to libobs/obs-module-type-rules.c (#1876, slice
# 5): the callbacks each output flag combination, encoder type and
# capability set and service needs, the texture encoder scaling rule and the
# protocol dedupe, through the four hidden functions compiled from the moved
# file; and obs_register_output_s(), obs_register_encoder_s() and
# obs_register_service_s() of the real libobs through the public API (the
# messages, the sizes, the order of the checks, the protocol merge and the
# freed type data), so the callers run against the C backend, the hybrid
# libobs and libobs-c-reference. obs_startup() needs a display on Linux, so
# the test runs under xvfb-run there.
# Target and test are declared in ../build/test-graph.json.
