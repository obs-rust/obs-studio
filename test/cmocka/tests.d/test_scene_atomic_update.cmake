# Exercise the public C API against whichever libobs backend the harness loads.
add_executable(test_scene_atomic_update test_scene_atomic_update.c)
target_include_directories(test_scene_atomic_update PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_scene_atomic_update PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
add_dependencies(cmocka-tests test_scene_atomic_update)
add_test(NAME test_scene_atomic_update COMMAND test_scene_atomic_update)
set_tests_properties(test_scene_atomic_update PROPERTIES TIMEOUT 30)
