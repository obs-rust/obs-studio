# An output started on a running video encoder must not wait for the
# encoder's next scheduled keyframe (obsproject/obs-studio#12069). Needs core
# video, so it runs (under xvfb-run) on Linux and skips without a display.
# The real_ cases load the built obs-x264 and obs-ffmpeg modules when they
# are part of the build, and skip encoders that are not available.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_encoder_join_keyframe test_encoder_join_keyframe.c)
  target_compile_definitions(
    test_encoder_join_keyframe
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  if(TARGET obs-x264)
    target_compile_definitions(
      test_encoder_join_keyframe
      PRIVATE
        OBS_X264_MODULE_PATH="$<TARGET_FILE:obs-x264>"
        OBS_X264_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/obs-x264/data"
    )
    add_dependencies(test_encoder_join_keyframe obs-x264)
  endif()
  if(TARGET obs-ffmpeg)
    target_compile_definitions(
      test_encoder_join_keyframe
      PRIVATE
        OBS_FFMPEG_MODULE_PATH="$<TARGET_FILE:obs-ffmpeg>"
        OBS_FFMPEG_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/obs-ffmpeg/data"
    )
    add_dependencies(test_encoder_join_keyframe obs-ffmpeg)
  endif()
  target_include_directories(test_encoder_join_keyframe PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_encoder_join_keyframe PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_encoder_join_keyframe libobs-opengl)
  add_dependencies(cmocka-tests test_encoder_join_keyframe)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_encoder_join_keyframe
    COMMAND test_encoder_join_keyframe
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_encoder_join_keyframe PROPERTIES TIMEOUT 180)
endif()
