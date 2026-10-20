# Hybrid MP4 split by time: split on the first keyframe at or after the split
# time (obsproject/obs-studio#14051). Builds obs-outputs' mp4-output.c into
# the test and calls its should_split(); no libobs start-up, no display.
if(TARGET OBS::outputs)
  add_executable(
    test_mp4_split_time
    test_mp4_split_time.c
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/mp4-mux.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-av1.c.h"
    $<$<BOOL:${ENABLE_HEVC}>:${CMAKE_SOURCE_DIR}/plugins/obs-outputs/rtmp-hevc.c.h>
  )
  target_include_directories(
    test_mp4_split_time
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-outputs"
  )
  # mp4-output.c's header set includes librtmp's, built like obs-outputs.
  target_compile_definitions(test_mp4_split_time PRIVATE USE_MBEDTLS CRYPTO)
  target_link_libraries(test_mp4_split_time PRIVATE OBS::libobs OBS::opts-parser OBS::bpm ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_mp4_split_time)
  add_test(test_mp4_split_time ${CMAKE_CURRENT_BINARY_DIR}/test_mp4_split_time)
endif()
