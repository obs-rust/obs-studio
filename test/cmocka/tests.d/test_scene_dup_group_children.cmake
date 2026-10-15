# Studio Mode with "Duplicate Sources" must copy the sources inside groups too
# (obsproject/obs-studio#4309, #6977). Each case forks a child with core video,
# so it runs (under xvfb-run) on Linux and skips without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_scene_dup_group_children test_scene_dup_group_children.c)
  target_compile_definitions(
    test_scene_dup_group_children
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_scene_dup_group_children PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_dup_group_children PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_scene_dup_group_children libobs-opengl)
  add_dependencies(cmocka-tests test_scene_dup_group_children)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_scene_dup_group_children
    COMMAND test_scene_dup_group_children
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_scene_dup_group_children PROPERTIES TIMEOUT 180)
endif()
