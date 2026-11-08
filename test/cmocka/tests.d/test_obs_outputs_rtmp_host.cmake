# Registered once in ../build/test-graph.json for both native consumers.
# Preserve the original optional MbedTLS send-result cases on every host.
set(HAVE_MBEDTLS_TEST_SDK FALSE)
if(TARGET MbedTLS::mbedtls)
  set(HAVE_MBEDTLS_TEST_SDK TRUE)
endif()
