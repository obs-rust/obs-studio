# linux-v4l2: the Virtual Camera can be restarted during a recording
# (obsproject/obs-studio#14054): its device is opened close-on-exec, so the
# recording's muxer does not keep a copy. Runs the real v4l2-output.c with
# ioctl() wrapped. The FD_CLOEXEC case needs no device; the restart case uses
# vivid's video output node and core video, and skips without them.
if(OS_LINUX AND TARGET OBS::v4l2 AND TARGET OBS::libobs-opengl)
  add_executable(
    test_v4l2_vcam_cloexec
    test_v4l2_vcam_cloexec.c
    "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2/vcam-format.c.h"
  )
  target_include_directories(
    test_v4l2_vcam_cloexec
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/plugins/linux-v4l2"
  )
  target_link_libraries(test_v4l2_vcam_cloexec PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  target_link_options(test_v4l2_vcam_cloexec PRIVATE "LINKER:--wrap=ioctl")
  # As test_v4l2_vcam_write: an ASan runtime's references to the wrapped
  # function resolve to this executable's __wrap_ioctl, which must be visible.
  set_target_properties(test_v4l2_vcam_cloexec PROPERTIES C_VISIBILITY_PRESET default)
  # A build tree has no libobs-opengl on the loader path, so load it by path.
  target_compile_definitions(test_v4l2_vcam_cloexec PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  add_dependencies(test_v4l2_vcam_cloexec libobs-opengl)
  add_dependencies(cmocka-tests test_v4l2_vcam_cloexec)
  find_program(OBS_TEST_XVFB_RUN xvfb-run)
  set(_vcam_cloexec_display_command "")
  if(OBS_TEST_XVFB_RUN)
    set(_vcam_cloexec_display_command "${OBS_TEST_XVFB_RUN}" -a "--server-args=-screen 0 1280x1024x24 -noreset")
  endif()
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_v4l2_vcam_cloexec
    COMMAND ${_vcam_cloexec_display_command} $<TARGET_FILE:test_v4l2_vcam_cloexec>
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_v4l2_vcam_cloexec PROPERTIES TIMEOUT 60)
endif()
