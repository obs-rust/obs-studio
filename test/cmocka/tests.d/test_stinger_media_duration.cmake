# A Stinger transition must play its whole clip when the clip's header gives
# a short duration or none (upstream obsproject/obs-studio#9755). Loads the
# built obs-transitions module and builds obs-ffmpeg-source.c directly, so
# the obs-ffmpeg module and its hardware encoder probes are not loaded.
# Needs core video, so it runs (under xvfb-run) on Linux and skips without a
# display.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
set(HAVE_TRANSITIONS_TEST_OWNER FALSE)
if(TARGET obs-transitions)
  set(HAVE_TRANSITIONS_TEST_OWNER TRUE)
endif()
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
