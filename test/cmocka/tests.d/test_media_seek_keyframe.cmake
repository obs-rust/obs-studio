# A Media Source seek must land on the requested time, not on the keyframe
# before it (obsproject/obs-websocket#1190). Plays generated clips through
# media-playback directly, so it needs no display or graphics.
# Registration is shared with Cargo in ../build/test-graph.d.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
