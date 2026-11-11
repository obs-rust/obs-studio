# Registration lives in build/test-graph.d; no duplicate executable.
set(HAVE_PYTHON_EMBED_TEST_SDK FALSE)
if(OS_LINUX OR OS_FREEBSD OR OS_OPENBSD)
  find_package(Python 3.8 QUIET COMPONENTS Development.Embed)
  if(Python_Development.Embed_FOUND)
    set(HAVE_PYTHON_EMBED_TEST_SDK TRUE)
  endif()
endif()
