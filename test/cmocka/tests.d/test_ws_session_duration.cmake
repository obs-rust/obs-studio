# The unchanged WebSocket helper fixture is registered by ../build/test-graph.d.
# Preserve the original optional SDK discovery before the shared graph runs.
if(NOT TARGET Qt6::Core)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
