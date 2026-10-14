# obs-websocket must send SourceFilterSettingsChanged when a filter's settings
# change. Builds the plugin's event handler straight from its sources against
# real libobs, with the frontend calls it makes stubbed in the test, so it runs
# where the plugin itself is off (Tier 2). Needs nlohmann_json for the plugin's
# headers. obs_startup() needs a display on Linux; there the cases skip without
# one. Not on Windows: the frontend stubs would redefine dllimport
# declarations.
set(_ws_src "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/src")
if(NOT TARGET nlohmann_json::nlohmann_json)
  find_package(nlohmann_json 3.11 QUIET)
endif()
if(OS_LINUX)
  find_package(X11 QUIET)
endif()
if(
  NOT OS_WINDOWS
  AND TARGET nlohmann_json::nlohmann_json
  AND EXISTS "${_ws_src}/eventhandler/EventHandler.cpp"
  AND (NOT OS_LINUX OR TARGET X11::X11)
)
  set(obs-websocket_VERSION 0.0.0)
  set(OBS_WEBSOCKET_RPC_VERSION 1)
  configure_file(
    "${_ws_src}/plugin-macros.h.in"
    "${CMAKE_CURRENT_BINARY_DIR}/ws-filter-settings-event/plugin-macros.generated.h"
  )

  file(GLOB _ws_event_handler_sources CONFIGURE_DEPENDS "${_ws_src}/eventhandler/EventHandler*.cpp")
  add_executable(
    test_ws_filter_settings_event
    test_ws_filter_settings_event.cpp
    ${_ws_event_handler_sources}
    "${_ws_src}/utils/Json.cpp"
    "${_ws_src}/utils/Obs_ArrayHelper.cpp"
    "${_ws_src}/utils/Obs_NumberHelper.cpp"
    "${_ws_src}/utils/Obs_ObjectHelper.cpp"
    "${_ws_src}/utils/Obs_VolumeMeter.cpp"
  )
  target_include_directories(
    test_ws_filter_settings_event
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${_ws_src}"
      "${_ws_src}/utils"
      "${CMAKE_SOURCE_DIR}/frontend/api"
      "${CMAKE_CURRENT_BINARY_DIR}/ws-filter-settings-event"
  )
  target_compile_features(test_ws_filter_settings_event PRIVATE cxx_std_17)
  target_link_libraries(
    test_ws_filter_settings_event
    PRIVATE OBS::libobs nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES} $<$<BOOL:${OS_LINUX}>:X11::X11>
  )
  add_dependencies(cmocka-tests test_ws_filter_settings_event)
  add_test(test_ws_filter_settings_event ${CMAKE_CURRENT_BINARY_DIR}/test_ws_filter_settings_event)
  unset(_ws_event_handler_sources)
endif()
unset(_ws_src)
