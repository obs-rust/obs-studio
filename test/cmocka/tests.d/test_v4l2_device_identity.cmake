# linux-v4l2: the Video Capture Device follows its camera, not a /dev/videoN
# number (obs-rust/obs-studio#2262, upstream obsproject/obs-studio#3824).
# Runs the real v4l2-input.c against a fake libv4l2, a fake udev monitor and a
# fake /dev and sysfs in the build tree, so it needs no camera, kernel module,
# root or display.
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(Libv4l2 REQUIRED)

  add_executable(
    test_v4l2_device_identity
    test_v4l2_device_identity.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-busy.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-controls.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-decoder.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-frame-action.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-framerate.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-helpers.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input-caps.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input.c.h"
  )
  # libv4l2 and libudev are not linked: the test defines v4l2_open,
  # v4l2_ioctl and friends, and the udev monitor's signal handler. The plugin
  # looks for nodes in V4L2_DEV_DIR and V4L2_SYSFS_ROOT, the fake ones.
  set(_v4l2_fixture "${CMAKE_CURRENT_BINARY_DIR}/test_v4l2_device_identity.d")
  target_compile_definitions(
    test_v4l2_device_identity
    PRIVATE
      HAVE_UDEV=1
      FIXTURE_DIR="${_v4l2_fixture}"
      V4L2_DEV_DIR="${_v4l2_fixture}/dev"
      V4L2_SYSFS_ROOT="${_v4l2_fixture}/sys"
  )
  unset(_v4l2_fixture)
  target_include_directories(
    test_v4l2_device_identity
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2"
      "$<TARGET_PROPERTY:Libv4l2::Libv4l2,INTERFACE_INCLUDE_DIRECTORIES>"
  )
  target_link_libraries(
    test_v4l2_device_identity
    PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_v4l2_device_identity)
  add_test(test_v4l2_device_identity ${CMAKE_CURRENT_BINARY_DIR}/test_v4l2_device_identity)
  set_tests_properties(test_v4l2_device_identity PROPERTIES TIMEOUT 60)
endif()
