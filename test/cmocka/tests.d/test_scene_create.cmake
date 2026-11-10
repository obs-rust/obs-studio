# Public-API constructor characterization against either actual libobs library.
add_executable(test_scene_create test_scene_create.c)
target_include_directories(test_scene_create PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_scene_create PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_scene_create)
add_test(NAME test_scene_create COMMAND test_scene_create)
set_tests_properties(test_scene_create PROPERTIES SKIP_RETURN_CODE 77 RUN_SERIAL TRUE)
