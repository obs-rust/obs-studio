# obs-websocket RemoveInput must remove the scene items of the input when the
# request returns, also in scenes that are not rendered (#1613). Compiles the
# plugin's input requests from the submodule against libobs; the test defines
# the frontend calls. Needs no video. Skipped when the submodule is not checked
# out or nlohmann_json is missing.
set(_ws "${CMAKE_SOURCE_DIR}/plugins/obs-websocket")
if(OS_LINUX AND EXISTS "${_ws}/src/requesthandler/RequestHandler_Inputs.cpp")
  find_package(nlohmann_json 3.11 QUIET)
  if(nlohmann_json_FOUND)
    # plugin-macros.generated.h, as the plugin's own CMakeLists makes it.
    set(obs-websocket_VERSION "0.0.0")
    set(OBS_WEBSOCKET_RPC_VERSION 1)
    configure_file(
      "${_ws}/src/plugin-macros.h.in"
      "${CMAKE_CURRENT_BINARY_DIR}/ws_remove_input/plugin-macros.generated.h"
    )
    add_executable(
      test_ws_remove_input
      test_ws_remove_input.cpp
      "${_ws}/src/requesthandler/RequestHandler_Inputs.cpp"
      "${_ws}/src/requesthandler/rpc/Request.cpp"
      "${_ws}/src/requesthandler/rpc/RequestResult.cpp"
      "${_ws}/src/utils/Json.cpp"
      "${_ws}/src/utils/Obs_ActionHelper.cpp"
      "${_ws}/src/utils/Obs_ArrayHelper.cpp"
      "${_ws}/src/utils/Obs_NumberHelper.cpp"
      "${_ws}/src/utils/Obs_ObjectHelper.cpp"
      "${_ws}/src/utils/Obs_SearchHelper.cpp"
    )
    target_include_directories(
      test_ws_remove_input
      PRIVATE
        ${CMOCKA_INCLUDE_DIR}
        "${_ws}/src"
        "${CMAKE_CURRENT_BINARY_DIR}/ws_remove_input"
        "${CMAKE_SOURCE_DIR}/frontend/api"
    )
    # The plugin still calls APIs libobs has deprecated.
    target_compile_options(test_ws_remove_input PRIVATE -Wno-error=deprecated-declarations)
    target_compile_features(test_ws_remove_input PRIVATE cxx_std_17)
    target_link_libraries(test_ws_remove_input PRIVATE OBS::libobs nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES})
    add_dependencies(cmocka-tests test_ws_remove_input)
    # obs_startup() needs a display on Linux; the cases skip without one.
    find_program(XVFB_RUN xvfb-run)
    if(XVFB_RUN)
      add_test(NAME test_ws_remove_input COMMAND ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_ws_remove_input)
    else()
      add_test(test_ws_remove_input ${CMAKE_CURRENT_BINARY_DIR}/test_ws_remove_input)
    endif()
  endif()
endif()
