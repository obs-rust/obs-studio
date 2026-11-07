# Registered once in ../build/test-graph.json; both native consumers use that graph.
if(TARGET OBS::alsa)
  find_package(ALSA REQUIRED)
endif()
