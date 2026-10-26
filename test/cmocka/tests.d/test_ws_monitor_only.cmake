# obs-websocket SetInputAudioMonitorType "Monitor Only" must keep the input out
# of the program mix on 33.0 (#2447, the API side of #2444): monitoring on and
# muted, and GetInputAudioMonitorType reports it back. Compiles the plugin's
# input requests from the submodule against libobs, as test_ws_remove_input
# does; the test defines the frontend calls. Mixing needs core video, so it
# runs (under xvfb-run) on Linux and skips without a display. Skipped when the
# submodule is not checked out or nlohmann_json is missing.
set(_ws "${CMAKE_SOURCE_DIR}/plugins/obs-websocket")
if(OS_LINUX AND TARGET OBS::libobs-opengl AND EXISTS "${_ws}/src/requesthandler/RequestHandler_Inputs.cpp")
  find_package(nlohmann_json 3.11 QUIET)
  if(nlohmann_json_FOUND)
    # plugin-macros.generated.h, as the plugin's own CMakeLists makes it.
    set(obs-websocket_VERSION "0.0.0")
    set(OBS_WEBSOCKET_RPC_VERSION 1)
    configure_file(
      "${_ws}/src/plugin-macros.h.in"
      "${CMAKE_CURRENT_BINARY_DIR}/ws_monitor_only/plugin-macros.generated.h"
    )
    add_executable(
      test_ws_monitor_only
      test_ws_monitor_only.cpp
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
      test_ws_monitor_only
      PRIVATE
        ${CMOCKA_INCLUDE_DIR}
        "${_ws}/src"
        "${CMAKE_CURRENT_BINARY_DIR}/ws_monitor_only"
        "${CMAKE_SOURCE_DIR}/frontend/api"
    )
    target_compile_definitions(test_ws_monitor_only PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
    # The plugin still calls APIs libobs has deprecated.
    target_compile_options(test_ws_monitor_only PRIVATE -Wno-error=deprecated-declarations)
    target_compile_features(test_ws_monitor_only PRIVATE cxx_std_17)
    target_link_libraries(test_ws_monitor_only PRIVATE OBS::libobs nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES})
    add_dependencies(test_ws_monitor_only libobs-opengl)
    add_dependencies(cmocka-tests test_ws_monitor_only)
    # obs_startup() needs a display on Linux; the cases skip without one.
    # libobs finds its effect files relative to the working directory.
    find_program(XVFB_RUN xvfb-run)
    if(XVFB_RUN)
      add_test(
        NAME test_ws_monitor_only
        COMMAND ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_ws_monitor_only
        WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
      )
    else()
      add_test(NAME test_ws_monitor_only COMMAND test_ws_monitor_only WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
    endif()
    set_tests_properties(test_ws_monitor_only PROPERTIES TIMEOUT 180)
  endif()
endif()
