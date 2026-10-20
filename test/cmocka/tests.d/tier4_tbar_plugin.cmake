# Test plugin for Tier 4's "tbar" phase (run_tier4.py --only tbar
# --test-plugin-dir <this build directory>): calls obs_frontend_get_tbar_position()
# and obs_frontend_release_tbar() on obs-websocket vendor requests. Built, not
# installed or run by ctest.
if(OS_LINUX AND ENABLE_FRONTEND AND ENABLE_WEBSOCKET)
  add_library(tier4-tbar MODULE tier4_tbar_plugin.c)
  target_include_directories(tier4-tbar PRIVATE "${CMAKE_SOURCE_DIR}/plugins/obs-websocket/lib")
  target_link_libraries(tier4-tbar PRIVATE OBS::libobs OBS::frontend-api)
  set_target_properties(tier4-tbar PROPERTIES PREFIX "")
  add_dependencies(cmocka-tests tier4-tbar)
endif()
