if(TARGET OBS::libobs-opengl)
  # Public API characterization, linked against whichever libobs Tier 2 selects.
  add_executable(test_scene_item_set_visibility test_scene_item_set_visibility.c)
  target_include_directories(test_scene_item_set_visibility PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_scene_item_set_visibility PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
  if(TARGET OBS::w32-pthreads)
    target_link_libraries(test_scene_item_set_visibility PRIVATE OBS::w32-pthreads)
  endif()
  target_compile_definitions(
    test_scene_item_set_visibility
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  add_dependencies(test_scene_item_set_visibility libobs-opengl)
  add_dependencies(cmocka-tests test_scene_item_set_visibility)
  find_program(OBS_VISIBILITY_XVFB_RUN xvfb-run)
  if(OBS_VISIBILITY_XVFB_RUN)
    add_test(
      NAME test_scene_item_set_visibility
      COMMAND ${OBS_VISIBILITY_XVFB_RUN} -a $<TARGET_FILE:test_scene_item_set_visibility>
    )
  else()
    add_test(NAME test_scene_item_set_visibility COMMAND $<TARGET_FILE:test_scene_item_set_visibility>)
  endif()
  set_tests_properties(
    test_scene_item_set_visibility
    PROPERTIES TIMEOUT 30 WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
endif()
