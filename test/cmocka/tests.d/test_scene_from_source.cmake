# Public-API source conversion characterization against either libobs library.
add_executable(test_scene_from_source test_scene_from_source.c)
target_include_directories(test_scene_from_source PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_scene_from_source PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_scene_from_source)
add_test(NAME test_scene_from_source COMMAND test_scene_from_source)
set_tests_properties(test_scene_from_source PROPERTIES SKIP_RETURN_CODE 77 RUN_SERIAL TRUE)
