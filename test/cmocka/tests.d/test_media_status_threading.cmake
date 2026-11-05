# Production state transitions and decoder flushes against concurrent status queries.
if(TARGET OBS::media-playback)
  add_executable(test_media_status_threading test_media_status_threading.c)
  target_include_directories(test_media_status_threading PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}")
  target_link_libraries(
    test_media_status_threading
    PRIVATE OBS::libobs OBS::media-playback FFmpeg::swscale FFmpeg::swresample ${CMOCKA_LIBRARIES}
  )
  if(NOT MSVC)
    target_link_libraries(test_media_status_threading PRIVATE m)
  endif()
  add_dependencies(cmocka-tests test_media_status_threading)
  add_test(NAME test_media_status_threading COMMAND test_media_status_threading)
  set_tests_properties(test_media_status_threading PROPERTIES TIMEOUT 30)
endif()
