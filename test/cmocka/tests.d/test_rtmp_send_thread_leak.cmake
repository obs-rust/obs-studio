# The RTMP send thread leaks the packet it is sending when the connection
# fails (upstream obsproject/obs-studio#9972). Builds the real rtmp-stream.c
# (with the obs-outputs sources it links against) into the test and runs its
# send thread on a socket pair; the output and encoder are fakes, so no
# network, display or encoder is needed.
if(TARGET OBS::outputs AND NOT WIN32)
  find_package(MbedTLS 3...<4 REQUIRED)
  find_package(ZLIB REQUIRED)
  find_package(jansson REQUIRED)
  add_executable(
    test_rtmp_send_thread_leak
    test_rtmp_send_thread_leak.c
    $<$<BOOL:${ENABLE_HEVC}>:${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-hevc.c.h>
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/flv-mux.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/net-if.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-av1.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/amf.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/cencode.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/hashswf.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/log.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/md5.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/parseurl.c"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/librtmp/rtmp.c"
  )
  target_compile_definitions(test_rtmp_send_thread_leak PRIVATE USE_MBEDTLS CRYPTO)
  target_compile_options(test_rtmp_send_thread_leak PRIVATE $<$<COMPILE_LANG_AND_ID:C,AppleClang,Clang>:-Wno-comma>)
  target_include_directories(
    test_rtmp_send_thread_leak
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs"
  )
  target_link_libraries(
    test_rtmp_send_thread_leak
    PRIVATE
      OBS::libobs
      OBS::happy-eyeballs
      MbedTLS::mbedtls
      ZLIB::ZLIB
      jansson::jansson
      ${CMOCKA_LIBRARIES}
      "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Foundation.framework>>"
      "$<$<PLATFORM_ID:Darwin>:$<LINK_LIBRARY:FRAMEWORK,Security.framework>>"
  )
  add_dependencies(cmocka-tests test_rtmp_send_thread_leak)
  add_test(test_rtmp_send_thread_leak ${CMAKE_CURRENT_BINARY_DIR}/test_rtmp_send_thread_leak)
  set_tests_properties(test_rtmp_send_thread_leak PROPERTIES TIMEOUT 30)
endif()
