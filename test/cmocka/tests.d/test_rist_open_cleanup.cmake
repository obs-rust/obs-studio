# Registered once in ../build/test-graph.d; both native consumers use that graph.
find_package(Librist QUIET)
set(HAVE_LIBRIST_TEST_SDK FALSE)
if(TARGET Librist::Librist)
  set(HAVE_LIBRIST_TEST_SDK TRUE)
endif()
