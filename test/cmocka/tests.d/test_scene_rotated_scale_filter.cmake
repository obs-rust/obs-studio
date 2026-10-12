# Scale filtering must keep working on rotated scene items
# (obsproject/obs-studio#12625). Renders scenes, so this needs core video and
# runs (under xvfb-run) on Linux only; the cases skip without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_scene_rotated_scale_filter test_scene_rotated_scale_filter.c)
  target_compile_definitions(
    test_scene_rotated_scale_filter
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_scene_rotated_scale_filter PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_rotated_scale_filter PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} m)
  add_dependencies(test_scene_rotated_scale_filter libobs-opengl)
  add_dependencies(cmocka-tests test_scene_rotated_scale_filter)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_scene_rotated_scale_filter
    COMMAND test_scene_rotated_scale_filter
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_scene_rotated_scale_filter PROPERTIES TIMEOUT 120)
endif()
