# Defined in build/test-graph.d; preserve the optional SDK requirement.
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_PULSEAUDIO)
  find_package(PulseAudio REQUIRED)
endif()
