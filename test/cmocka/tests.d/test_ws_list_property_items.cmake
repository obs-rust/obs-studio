# obs-websocket GetInputPropertiesListPropertyItems must answer, not abort
# OBS, when a list property holds an item with a NULL name or NULL string value
# (upstream obsproject/obs-studio#13905, obsproject/obs-websocket#1355).
# Compiles the plugin's ArrayHelper from the submodule against libobs; the test
# defines the frontend calls. Needs no libobs startup and no display. Skipped
# when the submodule is not checked out or nlohmann_json is missing.
set(_ws "${CMAKE_SOURCE_DIR}/plugins/obs-websocket")
if(OS_LINUX AND EXISTS "${_ws}/src/utils/Obs_ArrayHelper.cpp")
  if(NOT TARGET nlohmann_json::nlohmann_json)
    find_package(nlohmann_json 3.11 QUIET)
  endif()
  if(TARGET nlohmann_json::nlohmann_json)
    # plugin-macros.generated.h, as the plugin's own CMakeLists makes it.
    set(obs-websocket_VERSION "0.0.0")
    set(OBS_WEBSOCKET_RPC_VERSION 1)
    configure_file(
      "${_ws}/src/plugin-macros.h.in"
      "${CMAKE_CURRENT_BINARY_DIR}/ws_list_property_items/plugin-macros.generated.h"
    )
    add_executable(
      test_ws_list_property_items
      test_ws_list_property_items.cpp
      "${_ws}/src/utils/Json.cpp"
      "${_ws}/src/utils/Obs_ArrayHelper.cpp"
      "${_ws}/src/utils/Obs_NumberHelper.cpp"
      "${_ws}/src/utils/Obs_ObjectHelper.cpp"
    )
    target_include_directories(
      test_ws_list_property_items
      PRIVATE
        ${CMOCKA_INCLUDE_DIR}
        "${_ws}/src"
        "${CMAKE_CURRENT_BINARY_DIR}/ws_list_property_items"
        "${CMAKE_SOURCE_DIR}/frontend/api"
    )
    # The plugin still calls APIs libobs has deprecated.
    target_compile_options(test_ws_list_property_items PRIVATE -Wno-error=deprecated-declarations)
    target_compile_features(test_ws_list_property_items PRIVATE cxx_std_17)
    target_link_libraries(
      test_ws_list_property_items
      PRIVATE OBS::libobs nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_ws_list_property_items)
    add_test(test_ws_list_property_items ${CMAKE_CURRENT_BINARY_DIR}/test_ws_list_property_items)
  endif()
endif()
unset(_ws)
