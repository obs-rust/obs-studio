# Scene item crops and group sizes loaded from a scene collection out of
# range: a negative crop is clamped as obs_sceneitem_set_crop() does, and a
# crop or size past its type's range saturates. Loads scenes through the real
# libobs and renders them, so this needs core video and runs (under
# xvfb-run) on Linux only; the cases skip without a display.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_scene_item_crop_load test_scene_item_crop_load.c)
  target_compile_definitions(test_scene_item_crop_load PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_scene_item_crop_load PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_item_crop_load PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  add_dependencies(test_scene_item_crop_load libobs-opengl)
  add_dependencies(cmocka-tests test_scene_item_crop_load)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_scene_item_crop_load
    COMMAND test_scene_item_crop_load
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_scene_item_crop_load PROPERTIES TIMEOUT 120)
endif()
