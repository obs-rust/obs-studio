# Network media sources must give up on a silent connection so they
# reconnect (upstream obsproject/obs-studio#12958, #11537). The end-to-end
# case runs its own silent RTSP camera on 127.0.0.1; no server is needed.
# Registration is shared with Cargo in ../build/test-graph.d.
set(HAVE_MEDIA_PLAYBACK_TEST_OWNER FALSE)
if(TARGET OBS::media-playback)
  set(HAVE_MEDIA_PLAYBACK_TEST_OWNER TRUE)
endif()
