# Characterization uses private records only for the adapter's read-only guards.
add_executable(test_scene_group_enum test_scene_group_enum.c)
target_include_directories(test_scene_group_enum PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_scene_group_enum
  PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)
add_dependencies(cmocka-tests test_scene_group_enum)
add_test(NAME test_scene_group_enum COMMAND test_scene_group_enum)
set_tests_properties(
  test_scene_group_enum
  PROPERTIES SKIP_RETURN_CODE 77 RUN_SERIAL TRUE TIMEOUT 30
)
