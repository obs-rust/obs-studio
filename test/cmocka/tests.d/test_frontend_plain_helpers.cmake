# Header-only frontend helpers from ../build/test-graph.d need no Qt.
set(HAVE_FRONTEND_PLAIN_TEST_OWNER FALSE)
if(ENABLE_FRONTEND AND ENABLE_UNIT_TESTS)
  set(HAVE_FRONTEND_PLAIN_TEST_OWNER TRUE)
endif()
