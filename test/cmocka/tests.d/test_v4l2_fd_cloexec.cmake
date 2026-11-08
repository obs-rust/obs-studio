# Unchanged headless helper is registered in build/test-graph.json (Refs #1390).
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(Libv4l2 REQUIRED)
endif()
