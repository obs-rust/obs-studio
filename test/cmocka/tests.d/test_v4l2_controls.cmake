# Saved V4L2 camera controls are never applied (obsproject/obs-studio#11633):
# runs the real v4l2-input.c and v4l2-controls.c against a fake libv4l2 and a
# fake udev monitor in the test, so it needs no camera, kernel module or
# display.
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(Libv4l2 REQUIRED)

  add_executable(
    test_v4l2_controls
    test_v4l2_controls.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-controls.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-decoder.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-frame-action.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-framerate.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-helpers.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input-caps.c"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input.c"
  )
  # libv4l2 and libudev are not linked: the test defines v4l2_open,
  # v4l2_ioctl and friends, and the udev monitor's signal handler.
  target_compile_definitions(test_v4l2_controls PRIVATE HAVE_UDEV=1)
  target_include_directories(
    test_v4l2_controls
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2"
      "$<TARGET_PROPERTY:Libv4l2::Libv4l2,INTERFACE_INCLUDE_DIRECTORIES>"
  )
  target_link_libraries(
    test_v4l2_controls
    PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_v4l2_controls)
  add_test(test_v4l2_controls ${CMAKE_CURRENT_BINARY_DIR}/test_v4l2_controls)
  set_tests_properties(test_v4l2_controls PROPERTIES TIMEOUT 60)
endif()
