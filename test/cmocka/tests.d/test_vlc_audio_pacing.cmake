# VLC Video Source hands its audio over as it plays, not in one-second bursts
# (upstream obsproject/obs-studio#2406). Generates Opus files with FFmpeg and
# plays them through the built vlc-video module. The cases need core video
# (run under xvfb-run) and skip without a display or when libVLC is not
# installed; the test then exits with 77 and ctest reports it as skipped.
# A disabled module (ENABLE_VLC=OFF) still leaves a dummy custom target named
# vlc-video behind, which has no file, so check the target type too.
if(TARGET vlc-video)
  get_target_property(_vlc_video_type vlc-video TYPE)
endif()
if(OS_LINUX AND _vlc_video_type STREQUAL "MODULE_LIBRARY" AND TARGET OBS::libobs-opengl)
  add_executable(test_vlc_audio_pacing test_vlc_audio_pacing.c)
  target_compile_definitions(
    test_vlc_audio_pacing
    PRIVATE
      VLC_MODULE_PATH="$<TARGET_FILE:vlc-video>"
      GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
      VLC_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/vlc-video/data"
  )
  target_include_directories(test_vlc_audio_pacing PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(
    test_vlc_audio_pacing
    PRIVATE
      OBS::libobs
      FFmpeg::avcodec
      FFmpeg::avformat
      FFmpeg::avutil
      ${CMOCKA_LIBRARIES}
      $<$<NOT:$<PLATFORM_ID:Windows>>:m>
  )
  add_dependencies(test_vlc_audio_pacing vlc-video libobs-opengl)
  add_dependencies(cmocka-tests test_vlc_audio_pacing)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_vlc_audio_pacing COMMAND test_vlc_audio_pacing WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_vlc_audio_pacing PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 120)
endif()
