# Built-in services connect over RTMPS when their ingest server is known to
# accept it, and keep the plain URL for the frontend's fallback
# (obs-rust/obs-studio#2205). The cases load the real rtmp-services module, so
# they only run on Linux, and obs_startup needs a display there (run under
# xvfb-run, as Tier 2 does).
set(HAVE_RTMP_SERVICES_MODULE FALSE)
if(OS_LINUX AND TARGET rtmp-services)
  set(HAVE_RTMP_SERVICES_MODULE TRUE)
endif()
# Registered once in build/test-graph.d, consumed by CMake and Cargo.
