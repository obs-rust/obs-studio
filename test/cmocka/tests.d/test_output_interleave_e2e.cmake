# The output interleaver end to end through the libobs that ships, which
# test_output_interleave (it compiles obs-output-interleave.c.h into itself)
# does not reach: registered video and two audio encoders, an encoded output
# and media fed by hand, as in test_output_interleave_restart. It uses real
# core video, so it needs an X display on Linux (xvfb-run, as in CI).
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_output_interleave_e2e test_output_interleave_e2e.c)
  target_compile_definitions(
    test_output_interleave_e2e
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_output_interleave_e2e PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_output_interleave_e2e PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_output_interleave_e2e libobs-opengl)
  add_dependencies(cmocka-tests test_output_interleave_e2e)
  add_test(
    NAME test_output_interleave_e2e
    COMMAND test_output_interleave_e2e
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_output_interleave_e2e PROPERTIES TIMEOUT 60 SKIP_RETURN_CODE 77)
endif()
