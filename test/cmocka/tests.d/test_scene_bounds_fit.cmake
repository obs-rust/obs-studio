# A source with no size must get a finite bounding box scale
# (obsproject/obs-studio#4245). Builds libobs/obs-scene-bounds.c on its own:
# no display and no libobs core, so it runs on every platform.
add_executable(test_scene_bounds_fit test_scene_bounds_fit.c "${CMAKE_SOURCE_DIR}/libobs/obs-scene-bounds.c")
target_include_directories(test_scene_bounds_fit PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_scene_bounds_fit PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})
if(NOT MSVC)
  target_link_libraries(test_scene_bounds_fit PRIVATE m)
endif()

add_dependencies(cmocka-tests test_scene_bounds_fit)
add_test(test_scene_bounds_fit ${CMAKE_CURRENT_BINARY_DIR}/test_scene_bounds_fit)
