# Call the exported API from the selected real hybrid/C-reference libobs.
add_executable(test_enum_scenes test_enum_scenes.c)
target_include_directories(test_enum_scenes PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_enum_scenes PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
if(TARGET OBS::w32-pthreads)
  target_link_libraries(test_enum_scenes PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_enum_scenes)
if(XVFB_RUN)
  add_test(
    NAME test_enum_scenes
    COMMAND ${XVFB_RUN} -a -s "-screen 0 1280x1024x24 -noreset" $<TARGET_FILE:test_enum_scenes>
  )
else()
  add_test(NAME test_enum_scenes COMMAND test_enum_scenes)
endif()
set_tests_properties(test_enum_scenes PROPERTIES TIMEOUT 30)
