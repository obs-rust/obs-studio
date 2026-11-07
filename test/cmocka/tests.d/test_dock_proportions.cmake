# The unchanged nine dock cases live in build/test-graph.json (#1390).
# Preserve optional Linux/Qt discovery before the shared graph evaluates it.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
endif()
