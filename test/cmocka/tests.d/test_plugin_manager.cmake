# Real Idian owner for the Plugin Manager fixture in build/test-graph.json.
if(OS_LINUX AND (NOT DEFINED ENABLE_UI OR ENABLE_UI) AND (NOT DEFINED ENABLE_FRONTEND OR ENABLE_FRONTEND))
  find_package(Qt6 QUIET COMPONENTS Widgets Svg)
  find_package(nlohmann_json 3.11 QUIET)
  if(TARGET Qt6::Widgets AND TARGET Qt6::Svg AND TARGET nlohmann_json::nlohmann_json AND NOT TARGET OBS::idian)
    add_subdirectory("${CMAKE_SOURCE_DIR}/shared/qt/idian" "${CMAKE_BINARY_DIR}/shared/qt/idian")
  endif()
endif()
