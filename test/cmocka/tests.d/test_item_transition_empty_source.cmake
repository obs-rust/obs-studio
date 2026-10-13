# A cropped item's show/hide transition must run out while its source has no
# size (obsproject/obs-studio#10852). Item transitions run on the render
# thread, which needs core video, so it runs (under xvfb-run) on Linux only.
if(OS_LINUX AND TARGET OBS::libobs-opengl)
  add_executable(test_item_transition_empty_source test_item_transition_empty_source.c)
  target_compile_definitions(
    test_item_transition_empty_source
    PRIVATE GRAPHICS_MODULE_PATH="$<TARGET_FILE:OBS::libobs-opengl>"
  )
  target_include_directories(test_item_transition_empty_source PRIVATE ${CMOCKA_INCLUDE_DIR})
  target_link_libraries(test_item_transition_empty_source PRIVATE OBS::libobs OBS::caption ${CMOCKA_LIBRARIES})
  add_dependencies(test_item_transition_empty_source libobs-opengl)
  add_dependencies(cmocka-tests test_item_transition_empty_source)
  # libobs finds its effect files relative to the working directory.
  add_test(
    NAME test_item_transition_empty_source
    COMMAND test_item_transition_empty_source
    WORKING_DIRECTORY "${OBS_OUTPUT_DIR}/$<CONFIG>"
  )
  set_tests_properties(test_item_transition_empty_source PROPERTIES TIMEOUT 120)
endif()
