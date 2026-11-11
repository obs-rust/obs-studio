# A Media Source must follow a stream whose frame size or pixel format
# changes mid-file (upstream obsproject/obs-studio PR #5010). Plays generated
# clips through media-playback directly, so it needs no display or graphics.
# Registration is shared with Cargo in ../build/test-graph.d.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
