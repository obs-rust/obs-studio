# "Ignore streaming service setting recommendations" keeps the user's encoder
# settings in Advanced output (obsproject/obs-studio#4125). Builds the plain C
# helper the frontend calls, so the test does not link the Qt frontend. The
# stand_in_ cases need no display and run everywhere. On Linux the module_
# cases also load the real rtmp-services module; obs_startup needs a display
# there, so they skip without one (run under xvfb-run).
add_executable(
  test_service_encoder_settings
  test_service_encoder_settings.c
  "${CMAKE_SOURCE_DIR}/frontend/utility/service-encoder-settings.c"
)
target_include_directories(
  test_service_encoder_settings
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
)
target_link_libraries(test_service_encoder_settings PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(OS_LINUX AND TARGET rtmp-services)
  target_compile_definitions(
    test_service_encoder_settings
    PRIVATE
      RTMP_SERVICES_MODULE_PATH="$<TARGET_FILE:rtmp-services>"
      SERVICES_DATA_DIR="${CMAKE_CURRENT_BINARY_DIR}/test_service_encoder_settings_data"
  )
  target_include_directories(test_service_encoder_settings PRIVATE "${CMAKE_SOURCE_DIR}/plugins/rtmp-services")
  add_dependencies(test_service_encoder_settings rtmp-services)
endif()
add_dependencies(cmocka-tests test_service_encoder_settings)
add_test(NAME test_service_encoder_settings COMMAND test_service_encoder_settings)
set_tests_properties(test_service_encoder_settings PROPERTIES TIMEOUT 60)
