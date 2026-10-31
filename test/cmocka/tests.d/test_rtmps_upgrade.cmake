# Built-in services connect over RTMPS when their ingest server is known to
# accept it, and keep the plain URL for the frontend's fallback
# (obs-rust/obs-studio#2205). The cases load the real rtmp-services module, so
# they only run on Linux, and obs_startup needs a display there (run under
# xvfb-run, as Tier 2 does).
add_executable(test_rtmps_upgrade test_rtmps_upgrade.c)
target_include_directories(test_rtmps_upgrade PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_rtmps_upgrade PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(OS_LINUX AND TARGET rtmp-services)
  target_compile_definitions(
    test_rtmps_upgrade
    PRIVATE
      RTMP_SERVICES_MODULE_PATH="$<TARGET_FILE:rtmp-services>"
      SERVICES_DATA_DIR="${CMAKE_CURRENT_BINARY_DIR}/test_rtmps_upgrade_data"
  )
  target_include_directories(test_rtmps_upgrade PRIVATE "${CMAKE_SOURCE_DIR}/plugins/rtmp-services")
  add_dependencies(test_rtmps_upgrade rtmp-services)
endif()
add_dependencies(cmocka-tests test_rtmps_upgrade)
add_test(NAME test_rtmps_upgrade COMMAND test_rtmps_upgrade)
set_tests_properties(test_rtmps_upgrade PROPERTIES TIMEOUT 60)
