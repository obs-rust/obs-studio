# linux-v4l2: the Virtual Camera also sends OBS's audio to an "OBS Virtual
# Microphone" (obs-rust/obs-studio#3011). Runs the real v4l2-output.c against a
# real pipe, so no sound server or v4l2loopback device is needed.
if(OS_LINUX AND TARGET OBS::v4l2)
  add_executable(test_v4l2_vcam_audio test_v4l2_vcam_audio.c "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/vcam-format.c.h")
  target_include_directories(
    test_v4l2_vcam_audio
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2"
  )
  target_link_libraries(test_v4l2_vcam_audio PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(cmocka-tests test_v4l2_vcam_audio)
  add_test(test_v4l2_vcam_audio ${CMAKE_CURRENT_BINARY_DIR}/test_v4l2_vcam_audio)
  set_tests_properties(test_v4l2_vcam_audio PROPERTIES TIMEOUT 60)
endif()
