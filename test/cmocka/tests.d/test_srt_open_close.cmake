# obs-ffmpeg SRT output: a failed or rejected connection must release its
# libsrt reference exactly once, or libsrt breaks every other SRT socket in the
# process (SRT Media Sources). Runs its own SRT listener on 127.0.0.1; no
# server is needed.
if(OS_LINUX AND ENABLE_NEW_MPEGTS_OUTPUT AND TARGET OBS::ffmpeg)
  find_package(Libsrt QUIET)
  if(TARGET Libsrt::Libsrt)
    add_executable(test_srt_open_close test_srt_open_close.c)
    target_include_directories(
      test_srt_open_close
      PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg"
    )
    target_link_libraries(
      test_srt_open_close
      PRIVATE OBS::libobs FFmpeg::avformat FFmpeg::avutil Libsrt::Libsrt ${CMOCKA_LIBRARIES}
    )
    target_link_options(test_srt_open_close PRIVATE "-Wl,--wrap=srt_startup" "-Wl,--wrap=srt_cleanup")
    add_dependencies(cmocka-tests test_srt_open_close)
    add_test(NAME test_srt_open_close COMMAND test_srt_open_close)
    set_tests_properties(test_srt_open_close PROPERTIES TIMEOUT 60)
  endif()
endif()
