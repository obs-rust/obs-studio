# Characterize the selected actual hybrid/reference library before the Rust swap.
add_executable(test_scene_group_enum_items test_scene_group_enum_items.c)
target_include_directories(test_scene_group_enum_items PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_scene_group_enum_items
  PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)
add_dependencies(cmocka-tests test_scene_group_enum_items)
add_test(NAME test_scene_group_enum_items COMMAND test_scene_group_enum_items)
set_tests_properties(test_scene_group_enum_items PROPERTIES TIMEOUT 30)
