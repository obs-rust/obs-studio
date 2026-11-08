# Optional original SDK discovery; registration lives in the shared graph.
find_package(Qt6 QUIET COMPONENTS Core)
if(OS_LINUX)
  find_package(nlohmann_json 3.11 QUIET)
endif()
