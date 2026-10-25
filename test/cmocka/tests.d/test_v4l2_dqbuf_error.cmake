# linux-v4l2: a camera that fails without being unplugged (a VIDIOC_DQBUF
# error, no udev event) is closed and reopened, not left frozen on its last
# frame (obs-rust/obs-studio#2264, upstream obsproject/obs-studio#9434). Runs
# the real capture thread against a fake libv4l2 in the test, so it needs no
# camera, kernel module or display.
if(OS_LINUX AND TARGET OBS::v4l2)
  find_package(Libv4l2 REQUIRED)

  add_executable(
    test_v4l2_dqbuf_error
    test_v4l2_dqbuf_error.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-busy.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-controls.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-decoder.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-frame-action.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-framerate.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-helpers.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input-caps.c.h"
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/v4l2-input.c.h"
  )
  # libv4l2 is not linked: the test defines v4l2_open, v4l2_ioctl and friends.
  target_include_directories(
    test_v4l2_dqbuf_error
    PRIVATE
      ${CMOCKA_INCLUDE_DIR}
      "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2"
      "$<TARGET_PROPERTY:Libv4l2::Libv4l2,INTERFACE_INCLUDE_DIRECTORIES>"
  )
  target_link_libraries(
    test_v4l2_dqbuf_error
    PRIVATE OBS::libobs FFmpeg::avcodec FFmpeg::avformat FFmpeg::avutil ${CMOCKA_LIBRARIES}
  )
  add_dependencies(cmocka-tests test_v4l2_dqbuf_error)
  add_test(test_v4l2_dqbuf_error ${CMAKE_CURRENT_BINARY_DIR}/test_v4l2_dqbuf_error)
  set_tests_properties(test_v4l2_dqbuf_error PROPERTIES TIMEOUT 90)
endif()
