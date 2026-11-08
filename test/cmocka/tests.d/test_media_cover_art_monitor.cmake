# Audio monitoring of a Media Source with embedded cover art after the
# source restarts (upstream obsproject/obs-studio#9428, #11739): the
# monitor's video sync decision, fed by the real ffmpeg_source. Builds
# obs-ffmpeg-source.c directly so the obs-ffmpeg module and its hardware
# encoder probes are not loaded, and monitor-video-sync.c, which libobs builds
# only on Windows. Needs core video, so it runs (under
# xvfb-run) on Linux and skips without a display.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
# Registered once in build/test-graph.json, consumed by CMake and Cargo.
