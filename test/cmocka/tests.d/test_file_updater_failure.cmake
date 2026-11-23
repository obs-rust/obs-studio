# Production file-updater interface sources are explicit in ../build/test-graph.d.
if(OS_LINUX)
  find_package(CURL REQUIRED)
endif()
