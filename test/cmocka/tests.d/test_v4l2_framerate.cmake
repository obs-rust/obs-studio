# linux-v4l2 framerate list values (obsproject/obs-studio#13388, #13896) and
# framerate settings saved with the truncated value (obsproject/obs-studio#14036).
# Runs the real v4l2-input.c against a fake libv4l2 in the test, so it needs
# no camera, kernel module or display.
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(Libv4l2 REQUIRED)

  add_executable(
    test_v4l2_framerate
    test_v4l2_framerate.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-busy.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-controls.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-decoder.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-frame-action.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-framerate.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-helpers.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input-caps.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input.c"
  )
  # libv4l2 is not linked: the test defines v4l2_open, v4l2_ioctl and friends.
  target_include_directories(
    test_v4l2_framerate
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2"
      "$<TARGET_PROPERTY:Libv4l2::Libv4l2,INTERFACE_INCLUDE_DIRECTORIES>"
  )
  target_link_libraries(
    test_v4l2_framerate
    PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_v4l2_framerate)
  add_test(test_v4l2_framerate ${CMAKE_CURRENT_BINARY_DIR}/test_v4l2_framerate)
  set_tests_properties(test_v4l2_framerate PROPERTIES TIMEOUT 60)
endif()
