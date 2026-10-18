# GetStats reports the CPU usage of OBS since the same client's previous
# GetStats, with one locked CPU usage state per session (obs-rust/obs-studio#1346).
# Compiles the plugin's request handlers from the submodule against libobs and
# the frontend API library and sends requests through
# RequestHandler::ProcessRequest. Skipped when the submodule is not
# checked out, nlohmann_json or Qt is missing, or the frontend is not built.
set(_ws "${CMAKE_SOURCE_DIR}/plugins/obs-websocket")
# test/cmocka is configured before frontend/, so OBS::frontend-api is not a target
# yet here; it resolves when the build is generated.
if(OS_LINUX AND ENABLE_FRONTEND AND EXISTS "${_ws}/src/requesthandler/RequestHandler.cpp")
  find_package(nlohmann_json 3.11 QUIET)
  find_package(Threads REQUIRED)
  find_package(Qt6 QUIET COMPONENTS Core Network Widgets)
  if(nlohmann_json_FOUND AND TARGET Qt6::Core AND TARGET Qt6::Network AND TARGET Qt6::Widgets)
    # plugin-macros.generated.h, as the plugin's own CMakeLists makes it.
    set(obs-websocket_VERSION "0.0.0")
    set(OBS_WEBSOCKET_RPC_VERSION 1)
    configure_file("${_ws}/src/plugin-macros.h.in" "${CMAKE_CURRENT_BINARY_DIR}/ws_cpu_usage/plugin-macros.generated.h")
    file(
      GLOB _ws_cpu_sources
      "${_ws}/src/requesthandler/RequestHandler.cpp"
      "${_ws}/src/requesthandler/RequestHandler_*.cpp"
      "${_ws}/src/requesthandler/rpc/*.cpp"
      "${_ws}/src/utils/*.cpp"
    )
    add_executable(test_ws_cpu_usage test_ws_cpu_usage.cpp ${_ws_cpu_sources})
    target_include_directories(
      test_ws_cpu_usage
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${_ws}/src" "${_ws}/lib" "${CMAKE_CURRENT_BINARY_DIR}/ws_cpu_usage"
    )
    # The plugin still calls APIs libobs has deprecated.
    target_compile_options(test_ws_cpu_usage PRIVATE -Wno-error=deprecated-declarations)
    target_compile_features(test_ws_cpu_usage PRIVATE cxx_std_17)
    # GetStats reads the recording path from the frontend, which has no
    # callbacks here; the test answers that call itself.
    target_link_options(test_ws_cpu_usage PRIVATE -Wl,--wrap=obs_frontend_get_current_record_output_path)
    target_link_libraries(
      test_ws_cpu_usage
      PRIVATE
        OBS::libobs
        OBS::frontend-api
        nlohmann_json::nlohmann_json
        Qt6::Core
        Qt6::Network
        Qt6::Widgets
        Threads::Threads
        ${CMOCKA_LIBRARIES}
    )
    add_dependencies(cmocka-tests test_ws_cpu_usage)
    # obs_startup() needs a display on Linux; the cases skip without one.
    find_program(XVFB_RUN xvfb-run)
    if(XVFB_RUN)
      add_test(NAME test_ws_cpu_usage COMMAND ${XVFB_RUN} -a ${CMAKE_CURRENT_BINARY_DIR}/test_ws_cpu_usage)
    else()
      add_test(test_ws_cpu_usage ${CMAKE_CURRENT_BINARY_DIR}/test_ws_cpu_usage)
    endif()
  endif()
endif()
