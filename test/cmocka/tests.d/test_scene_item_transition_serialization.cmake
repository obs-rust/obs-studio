# Characterization of the real libobs serialization API. Borrowed internal
# fixtures need no running core, GPU, display, or worker threads.
add_executable(test_scene_item_transition_serialization test_scene_item_transition_serialization.c)
target_include_directories(test_scene_item_transition_serialization PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}/deps/libcaption")
target_link_libraries(test_scene_item_transition_serialization PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(OS_WINDOWS)
  target_link_libraries(test_scene_item_transition_serialization PRIVATE OBS::w32-pthreads)
endif()
add_dependencies(cmocka-tests test_scene_item_transition_serialization)
add_test(NAME test_scene_item_transition_serialization COMMAND test_scene_item_transition_serialization)
set_tests_properties(test_scene_item_transition_serialization PROPERTIES TIMEOUT 30)
