# Characterization of the scene item visibility audio ramp in
# libobs/obs-scene.c before its port: the sample of the program mix on which
# a show or hide lands, and the active references the applied hides drop.
# Public API plus the item and source refcounts, so it also covers the hybrid
# libobs. Applying the actions needs core video and audio, so it runs (under
# xvfb-run) on Linux only.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_scene_audio_actions test_scene_audio_actions.c)
  target_compile_definitions(test_scene_audio_actions PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>")
  target_include_directories(test_scene_audio_actions PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_audio_actions PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(test_scene_audio_actions libobs-opengl)
  add_dependencies(cmocka-tests test_scene_audio_actions)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_scene_audio_actions
    COMMAND test_scene_audio_actions
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_scene_audio_actions PROPERTIES TIMEOUT 180)
endif()
