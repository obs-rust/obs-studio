# linux-v4l2: the Video Capture Device follows its camera, not a /dev/videoN
# number (obs-rust/obs-studio#2262, upstream obsproject/obs-studio#3824), end
# to end with real libobs, the built linux-v4l2 module and the kernel's vivid
# test driver. Needs root (it loads vivid and unbinds/binds it), no other
# video device and a display (run under xvfb-run); otherwise it exits 77 and
# ctest reports it skipped. test_v4l2_device_identity covers the same cases
# with fakes.
if(OS_LINUX AND TARGET OBS::v4l2 AND TARGET OBS::libobs-opengl)
  add_executable(test_v4l2_vivid_replug test_v4l2_vivid_replug.c)
  target_compile_definitions(
    test_v4l2_vivid_replug
    PRIVATE
      GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
      V4L2_MODULE_PATH="$<TARGET_FILE:OBS::v4l2>"
      V4L2_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/data"
  )
  target_include_directories(test_v4l2_vivid_replug PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_v4l2_vivid_replug PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_v4l2_vivid_replug linux-v4l2 libobs-opengl)
  add_dependencies(cmocka-tests test_v4l2_vivid_replug)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_v4l2_vivid_replug COMMAND test_v4l2_vivid_replug WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_v4l2_vivid_replug PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 180)
endif()
