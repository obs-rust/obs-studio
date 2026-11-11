# Unchanged recording shutdown cases live in build/test-graph.d (#1390).
# Keep optional QtCore discovery under the original Linux guard.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
