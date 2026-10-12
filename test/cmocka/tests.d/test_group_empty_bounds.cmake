# Adding a source to an empty group with a bounding box must not give it a
# NaN transform (obsproject/obs-studio#4245). Renders scenes, so this needs
# core video and runs (under xvfb-run) on Linux only; the cases skip without
# a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_group_empty_bounds test_group_empty_bounds.c)
  target_compile_definitions(test_group_empty_bounds PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_group_empty_bounds PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_group_empty_bounds PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_group_empty_bounds libobs-opengl)
  add_dependencies(cmocka-tests test_group_empty_bounds)
  # libobs finds its effect files relative to the working directory.
  add_test(NAME test_group_empty_bounds COMMAND test_group_empty_bounds WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>")
  set_tests_properties(test_group_empty_bounds PROPERTIES TIMEOUT 120)
endif()
