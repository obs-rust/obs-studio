# The scene item crop and size arithmetic in libobs/obs-scene.h and
# libobs/obs-scene-crop.c.h on its own, with UBSan: crops and sizes from a
# scene collection out of range saturate, and nothing overflows. The crop
# functions are hidden in libobs, so this builds the moved file itself: no
# display and no libobs core, so it runs on every platform.
# test_scene_item_crop_load covers the callers through the real libobs.
add_executable(test_scene_crop_steps test_scene_crop_steps.c "${CMAKE_SOURCE_DIR}/libobs/obs-scene-crop.c.h")
target_include_directories(
  test_scene_crop_steps
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(test_scene_crop_steps PRIVATE OBS::libobs OBS::caption Uthash::Uthash ${CMOCKA_LIBRARIES})
# GCC leaves float-cast-overflow out of "undefined".
obs_test_sanitize(test_scene_crop_steps "undefined,float-cast-overflow")

add_dependencies(cmocka-tests test_scene_crop_steps)
add_test(test_scene_crop_steps ${CMAKE_CURRENT_BINARY_DIR}/test_scene_crop_steps)
