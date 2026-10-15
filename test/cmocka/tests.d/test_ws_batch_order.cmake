# obs-websocket must return a Parallel request batch's results in request
# order: the protocol layer labels results[i] with requests[i]'s requestType
# and requestId. Builds the plugin's batch handler straight from its sources,
# with RequestHandler stubbed in the test, so it runs where the plugin itself
# is off (Tier 2). Needs Qt Core (QThreadPool) and nlohmann_json for the
# plugin's headers; no libobs startup and no display. Not on Windows, like
# the other test_ws_* tests.
set(_ws_src "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/src")
if(NOT TARGET nlohmann_json::nlohmann_json)
  find_package(nlohmann_json 3.11 QUIET)
endif()
if(NOT TARGET Qt6::Core)
  find_package(Qt6 QUIET COMPONENTS Core)
endif()
if(
  NOT OS_WINDOWS
  AND TARGET nlohmann_json::nlohmann_json
  AND TARGET Qt6::Core
  AND EXISTS "${_ws_src}/requesthandler/RequestBatchHandler.cpp"
)
  set(obs-websocket_VERSION 0.0.0)
  set(OBS_WEBSOCKET_RPC_VERSION 1)
  configure_file("${_ws_src}/plugin-macros.h.in" "${CMAKE_CURRENT_BINARY_DIR}/ws-batch-order/plugin-macros.generated.h")

  add_executable(
    test_ws_batch_order
    test_ws_batch_order.cpp
    "${_ws_src}/requesthandler/RequestBatchHandler.cpp"
    "${_ws_src}/requesthandler/rpc/Request.cpp"
    "${_ws_src}/requesthandler/rpc/RequestBatchRequest.cpp"
    "${_ws_src}/requesthandler/rpc/RequestResult.cpp"
    "${_ws_src}/utils/Compat.cpp"
  )
  target_include_directories(
    test_ws_batch_order
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${_ws_src}"
      "${_ws_src}/utils"
      "${CMAKE_SOURCE_DIR}/frontend/api"
      "${CMAKE_CURRENT_BINARY_DIR}/ws-batch-order"
  )
  target_compile_features(test_ws_batch_order PRIVATE cxx_std_17)
  target_link_libraries(
    test_ws_batch_order
    PRIVATE OBS::libobs Qt6::Core nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_ws_batch_order)
  add_test(test_ws_batch_order ${CMAKE_CURRENT_BINARY_DIR}/test_ws_batch_order)
  cmocka_qt_runtime(test_ws_batch_order Qt6::Core)
endif()
unset(_ws_src)
