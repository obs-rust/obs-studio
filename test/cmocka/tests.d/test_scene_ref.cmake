# Characterize exported scene ref APIs against the selected actual libobs.
add_executable(test_scene_ref test_scene_ref.c)
target_include_directories(test_scene_ref PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_scene_ref PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_scene_ref)
if(XVFB_RUN)
  add_test(
    NAME test_scene_ref
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_scene_ref>
  )
else()
  add_test(NAME test_scene_ref COMMAND test_scene_ref)
endif()
set_tests_properties(test_scene_ref PROPERTIES TIMEOUT 30)
