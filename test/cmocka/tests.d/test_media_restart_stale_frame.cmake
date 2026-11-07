# Media Source restarting on activation must not flash the previous
# playback's last frame (upstream obsproject/obs-studio#4667). Builds
# obs-ffmpeg-source.c directly so the obs-ffmpeg module and its hardware
# encoder probes are not loaded. Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
