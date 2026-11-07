# Unchanged headless VPL fixture is registered in build/test-graph.json (Refs #1390).
set(HAVE_QSV_TEST_SDK FALSE)
if(OS_LINUX)
  find_package(VPL 2.9 QUIET)
  if(TARGET VPL::VPL)
    set(HAVE_QSV_TEST_SDK TRUE)
  endif()
endif()
