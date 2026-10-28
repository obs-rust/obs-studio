# plugins/obs-outputs: the RTMP output's POSIX host (#851), and the send
# result rules the Windows socket loop shares with it (#2220).
if(NOT OS_WINDOWS)
  add_executable(test_obs_outputs_rtmp_host test_obs_outputs_rtmp_host.c)
  target_include_directories(test_obs_outputs_rtmp_host PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}")
  target_link_libraries(test_obs_outputs_rtmp_host PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  # rtmp-send-result.h names the Mbed TLS errors the loop classifies. The cases
  # are skipped, not failed, in a configure without Mbed TLS.
  if(TARGET MbedTLS::mbedtls)
    target_link_libraries(test_obs_outputs_rtmp_host PRIVATE MbedTLS::mbedtls)
    target_compile_definitions(test_obs_outputs_rtmp_host PRIVATE TEST_HAVE_MBEDTLS)
  endif()
  add_dependencies(cmocka-tests test_obs_outputs_rtmp_host)
  add_test(test_obs_outputs_rtmp_host ${CMAKE_CURRENT_BINARY_DIR}/test_obs_outputs_rtmp_host)
endif()
