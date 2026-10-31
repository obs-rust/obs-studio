# linux-v4l2: the OBS Virtual Microphone against a private pipewire-pulse
# (obs-rust/obs-studio#3186). Skipped when the sound server tools are missing.
if(OS_LINUX AND TARGET OBS::v4l2)
  add_executable(test_v4l2_vmic_server test_v4l2_vmic_server.c "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/vcam-format.c.h")
  target_include_directories(
    test_v4l2_vmic_server
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2"
  )
  target_link_libraries(test_v4l2_vmic_server PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(cmocka-tests test_v4l2_vmic_server)
  add_test(test_v4l2_vmic_server ${CMAKE_CURRENT_BINARY_DIR}/test_v4l2_vmic_server)
  set_tests_properties(test_v4l2_vmic_server PROPERTIES TIMEOUT 120)
endif()
