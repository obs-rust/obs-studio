# A Media Source playing a WebM with alpha from a URL showed no video: the
# keyframe gate for network inputs waited for AV_FRAME_FLAG_KEY, which
# FFmpeg's libvpx decoders never set. Plays generated clips through
# media-playback directly, so it needs no display or graphics.
# Registration is shared with Cargo in ../build/test-graph.json.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
