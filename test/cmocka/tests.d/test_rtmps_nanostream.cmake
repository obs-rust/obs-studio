# Registered once in ../build/test-graph.d; both native consumers use that graph.
set(HAVE_RTMPS_TEST_SDK FALSE)
if(TARGET OBS::file-updater AND TARGET jansson::jansson)
  set(HAVE_RTMPS_TEST_SDK TRUE)
endif()
