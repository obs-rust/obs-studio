# The unchanged nine dock geometry cases retain their Linux/Widgets/Test guard,
# 120-second timeout and timing resource lock in ../build/test-graph.json.
if(OS_LINUX)
  find_package(Qt6 QUIET COMPONENTS Widgets Test)
endif()
# Target and runtime contract: ../build/test-graph.json (canonical).
