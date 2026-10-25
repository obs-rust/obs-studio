# linux-v4l2: a camera that fails without being unplugged is reopened, not
# left frozen (obs-rust/obs-studio#2264, upstream obsproject/obs-studio#9434),
# end to end with real libobs, the built linux-v4l2 module and the kernel's
# vivid test driver (its Disconnect control: ENODEV, no udev event). Needs
# root, vivid not already loaded and a display (run under xvfb-run);
# otherwise it exits 77 and ctest reports it skipped. test_v4l2_dqbuf_error
# covers the same with a fake device.
if(OS_LINUX AND TARGET OBS::v4l2 AND TARGET OBS::libobs-opengl)
  find_package(Libudev QUIET)
  if(TARGET Libudev::Libudev)
    add_executable(test_v4l2_vivid_stall test_v4l2_vivid_stall.c)
    target_compile_definitions(
      test_v4l2_vivid_stall
      PRIVATE
        GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
        V4L2_MODULE_PATH="$<TARGET_FILE:OBS::v4l2>"
        V4L2_DATA_PATH="${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/data"
    )
    target_include_directories(test_v4l2_vivid_stall PRIVATE ${CMOCKA_INCLUDE_DIR})
    target_link_libraries(test_v4l2_vivid_stall PRIVATE OBS::libobs Libudev::Libudev ${CMOCKA_LIBRARIES})
    add_dependencies(test_v4l2_vivid_stall linux-v4l2 libobs-opengl)
    add_dependencies(cmocka-tests test_v4l2_vivid_stall)
    # libobs finds its effect files relative to the working directory.
    add_test(NAME test_v4l2_vivid_stall COMMAND test_v4l2_vivid_stall WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
    set_tests_properties(test_v4l2_vivid_stall PROPERTIES SKIP_RETURN_CODE 77 TIMEOUT 120)
  endif()
endif()
