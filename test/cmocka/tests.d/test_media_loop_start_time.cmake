# A looping Media Source must not leave a gap between the loops when the file
# does not start at 0 (obsproject/obs-studio#12028). Plays generated clips
# through media-playback directly, so it needs no display or graphics.
# Registration is shared with Cargo in ../build/test-graph.d.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
