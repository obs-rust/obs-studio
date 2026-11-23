# Preserved C++ obs-websocket helper; the shared graph owns its build contract.
if(NOT TARGET nlohmann_json::nlohmann_json)
  find_package(nlohmann_json 3.11 QUIET)
endif()
# Registration lives in build/test-graph.d, consumed once by CMakeLists.txt.
