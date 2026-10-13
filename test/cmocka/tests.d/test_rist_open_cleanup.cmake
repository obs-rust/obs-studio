# RIST credential lifetime after open failure, without a sender or network.
find_package(Librist QUIET)
if(TARGET Librist::Librist)
  add_executable(test_rist_open_cleanup test_rist_open_cleanup.c)
  target_include_directories(
    test_rist_open_cleanup
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
  )
  target_link_libraries(
    test_rist_open_cleanup
    PRIVATE OBS::libobs Librist::Librist FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_rist_open_cleanup)
  add_test(test_rist_open_cleanup ${CMAKE_CURRENT_BINARY_DIR}/test_rist_open_cleanup)
endif()
