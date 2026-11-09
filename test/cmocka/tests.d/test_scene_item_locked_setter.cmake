add_executable(test_scene_item_locked_setter test_scene_item_locked_setter.c)
target_include_directories(test_scene_item_locked_setter PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_scene_item_locked_setter
  PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)
add_dependencies(cmocka-tests test_scene_item_locked_setter)
add_test(NAME test_scene_item_locked_setter COMMAND test_scene_item_locked_setter)
set_tests_properties(test_scene_item_locked_setter PROPERTIES TIMEOUT 30)
