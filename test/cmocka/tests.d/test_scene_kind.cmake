# Pin both exported kind predicates against the selected actual libobs.
add_executable(test_scene_kind test_scene_kind.c)
target_include_directories(test_scene_kind PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(
  test_scene_kind
  PRIVATE OBS::libobs ${CMOCKA_LIBRARIES} OBS::caption Uthash::Uthash $<$<PLATFORM_ID:Windows>:OBS::w32-pthreads>
)
add_dependencies(cmocka-tests test_scene_kind)
add_test(NAME test_scene_kind COMMAND test_scene_kind)
set_tests_properties(test_scene_kind PROPERTIES TIMEOUT 30)
