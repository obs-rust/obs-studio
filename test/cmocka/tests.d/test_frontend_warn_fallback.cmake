# Registration is shared with Cargo in ../build/test-graph.json.
set(HAVE_FRONTEND_WARN_TEST_OWNER FALSE)
if(OS_LINUX AND ENABLE_FRONTEND)
  set(HAVE_FRONTEND_WARN_TEST_OWNER TRUE)
endif()
