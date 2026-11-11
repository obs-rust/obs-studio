# Target, sanitizer selection and direct test command live in build/test-graph.d.
# Retain the original optional package lookup under the original guard.
if((OS_LINUX OR OS_FREEBSD) AND ENABLE_PULSEAUDIO)
  find_package(PulseAudio REQUIRED)
endif()
