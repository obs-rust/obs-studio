# Registered once in ../build/test-graph.d; both native consumers use that graph.
find_package(CURL QUIET)
set(HAVE_CURL_TEST_SDK FALSE)
if(TARGET CURL::libcurl)
  set(HAVE_CURL_TEST_SDK TRUE)
endif()
