# Private scenes and Studio Mode's private scene duplicates get the canvas of
# the scene they copy (upstream obsproject/obs-studio PR #12824). Needs core
# video for a second canvas, so it runs (under xvfb-run) on Linux only.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_scene_private_canvas test_scene_private_canvas.c)
  target_compile_definitions(test_scene_private_canvas PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_scene_private_canvas PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_private_canvas PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_scene_private_canvas libobs-opengl)
  add_dependencies(cmocka-tests test_scene_private_canvas)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_scene_private_canvas
    COMMAND test_scene_private_canvas
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_scene_private_canvas PROPERTIES TIMEOUT 120)
endif()
