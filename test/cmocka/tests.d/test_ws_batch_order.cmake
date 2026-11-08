# Preserved C++ obs-websocket helper; the shared graph owns its build contract.
if(NOT TARGET nlohmann_json::nlohmann_json)
  find_package(nlohmann_json 3.11 QUIET)
endif()
if(NOT TARGET Qt6::Core)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
include("${CMAKE_CURRENT_LIST_DIR}/../native-test-graph.cmake")
obs_native_test_graph("${CMAKE_CURRENT_LIST_DIR}/../build/test-graph.json" "${CMAKE_SOURCE_DIR}")
