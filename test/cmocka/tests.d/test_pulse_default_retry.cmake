# Unchanged cases and linker wrappers live in build/test-graph.d (#1390).
# Retain the original Linux/plugin guard; an absent optional SDK selects nothing.
set(HAVE_PULSE_DEFAULT_RETRY_SDK FALSE)
if(OS_LINUX AND TARGET OBS::pulseaudio)
  find_package(PulseAudio QUIET)
  if(TARGET PulseAudio::PulseAudio)
    set(HAVE_PULSE_DEFAULT_RETRY_SDK TRUE)
  endif()
endif()
