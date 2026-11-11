# Registered once in ../build/test-graph.d; both native consumers use that graph.
if(OS_LINUX AND ENABLE_NEW_MPEGTS_OUTPUT AND TARGET OBS::ffmpeg)
  find_package(Libsrt QUIET)
endif()
set(HAVE_SRT_TEST_SDK FALSE)
if(OS_LINUX AND ENABLE_NEW_MPEGTS_OUTPUT AND TARGET OBS::ffmpeg AND TARGET Libsrt::Libsrt)
  set(HAVE_SRT_TEST_SDK TRUE)
endif()
