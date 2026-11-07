# Registered once in ../build/test-graph.json; both native consumers use that graph.
if(TARGET OBS::libobs AND TARGET FFmpeg::avformat)
  find_package(FFmpeg 8.0 REQUIRED avdevice)
endif()
set(HAVE_MEDIA_PACKET_TEST_SDK FALSE)
if(TARGET OBS::libobs AND TARGET FFmpeg::avformat)
  set(HAVE_MEDIA_PACKET_TEST_SDK TRUE)
endif()
