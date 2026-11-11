# Public-API selection characterization against either actual libobs library.
add_executable(test_scene_item_select test_scene_item_select.c)
target_include_directories(test_scene_item_select PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_scene_item_select PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_scene_item_select)
add_test(NAME test_scene_item_select COMMAND test_scene_item_select)
set_tests_properties(test_scene_item_select PROPERTIES SKIP_RETURN_CODE 77 RUN_SERIAL TRUE)
