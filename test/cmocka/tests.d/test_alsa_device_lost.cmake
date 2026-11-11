# Registered once in ../build/test-graph.d; both native consumers use that graph.
if(TARGET OBS::alsa)
  find_package(ALSA REQUIRED)
endif()
