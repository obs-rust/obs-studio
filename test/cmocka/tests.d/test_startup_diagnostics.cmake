# Registrations live in ../build/test-graph.d (Refs #1390).
set(HAVE_STARTUP_GLAD FALSE)
if(TARGET OBS::glad)
  set(HAVE_STARTUP_GLAD TRUE)
endif()
