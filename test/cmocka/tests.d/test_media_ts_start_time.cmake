# A Media Source playing a file whose timestamps do not start at 0 (MPEG-TS)
# reported the media time with the start time added and seeked start-time
# early. Plays generated clips through media-playback directly, so it needs
# no display or graphics.
# Registration is shared with Cargo in ../build/test-graph.json.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
