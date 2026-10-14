# obs-websocket's RemoveScene must refuse to remove the last scene; it counts
# the main canvas's scenes with Utils::Obs::NumberHelper. Builds that helper
# straight from the plugin sources against stubbed libobs calls (no libobs
# startup, no display), so it runs where the plugin itself is off (Tier 2).
# Needs nlohmann_json for the plugin's headers. Not on Windows: the stubs
# would redefine libobs's dllimport declarations.
set(_ws_src "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/src")
if(NOT TARGET nlohmann_json::nlohmann_json)
  find_package(nlohmann_json 3.11 QUIET)
endif()
if(NOT OS_WINDOWS AND TARGET nlohmann_json::nlohmann_json AND EXISTS "${_ws_src}/utils/Obs_NumberHelper.cpp")
  set(obs-websocket_VERSION 0.0.0)
  set(OBS_WEBSOCKET_RPC_VERSION 1)
  configure_file("${_ws_src}/plugin-macros.h.in" "${CMAKE_CURRENT_BINARY_DIR}/ws-scene-count/plugin-macros.generated.h")

  add_executable(test_ws_scene_count test_ws_scene_count.cpp "${_ws_src}/utils/Obs_NumberHelper.cpp")
  target_include_directories(
    test_ws_scene_count
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${_ws_src}"
      "${_ws_src}/utils"
      "${CMAKE_SOURCE_DIR}/frontend/api"
      "${CMAKE_CURRENT_BINARY_DIR}/ws-scene-count"
      $<TARGET_PROPERTY:OBS::libobs,INTERFACE_INCLUDE_DIRECTORIES>
  )
  target_compile_definitions(test_ws_scene_count PRIVATE $<TARGET_PROPERTY:OBS::libobs,INTERFACE_COMPILE_DEFINITIONS>)
  target_compile_features(test_ws_scene_count PRIVATE cxx_std_17)
  # Make an uninitialized counter fail every run instead of depending on
  # what the stack held.
  include(CheckCXXCompilerFlag)
  check_cxx_compiler_flag(-ftrivial-auto-var-init=pattern HAVE_TRIVIAL_AUTO_VAR_INIT_PATTERN)
  if(HAVE_TRIVIAL_AUTO_VAR_INIT_PATTERN)
    target_compile_options(test_ws_scene_count PRIVATE -ftrivial-auto-var-init=pattern)
  endif()
  target_link_libraries(test_ws_scene_count PRIVATE nlohmann_json::nlohmann_json ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_ws_scene_count)
  add_test(test_ws_scene_count ${CMAKE_CURRENT_BINARY_DIR}/test_ws_scene_count)
endif()
unset(_ws_src)
