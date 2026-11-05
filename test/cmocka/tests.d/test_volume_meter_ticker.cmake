# The unchanged Linux/QtCore ticker cases live in build/test-graph.json (#1390).
# Preserve the original optional Qt discovery before the shared graph evaluates it.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
