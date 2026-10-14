# Show / Hide Transition from the Sources context menu reaches one of the
# selected sources only (obsproject/obs-studio#10448). Builds the plain C
# helper the frontend calls, so the test does not link the Qt frontend.
# Transitions need graphics, so it loads the built OpenGL module and runs (under
# xvfb-run) on Linux only; without a display the cases skip.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(
    test_visibility_transition
    test_visibility_transition.c
    "${CMAKE_SOURCE_DIR}/frontend/utility/visibility-transition.c"
  )
  target_compile_definitions(
    test_visibility_transition
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(
    test_visibility_transition
    PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/frontend/utility"
  )
  target_link_libraries(test_visibility_transition PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_visibility_transition libobs-opengl)
  add_dependencies(cmocka-tests test_visibility_transition)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_visibility_transition
    COMMAND test_visibility_transition
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
endif()
