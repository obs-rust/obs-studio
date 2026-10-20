# Characterization of the scene item blend mode table in
# libobs/obs-scene-blend.c before its port (#1520). The table is static, so the
# test compiles its file: no graphics device and no libobs core, so it runs on
# every platform. The file's precompiled header is libobs.pch.h, which needs
# the private libobs, libcaption, uthash and zlib headers.
add_executable(test_scene_blend test_scene_blend.c)
target_include_directories(
  test_scene_blend
  PRIVATE ${CMOCKA_INCLUDE_DIR} "${CMAKE_SOURCE_DIR}" "${CMAKE_SOURCE_DIR}/libobs" "${CMAKE_SOURCE_DIR}/deps/libcaption"
)
target_link_libraries(
  test_scene_blend
  PRIVATE OBS::libobs OBS::caption Uthash::Uthash ZLIB::ZLIB ${CMOCKA_LIBRARIES}
)

add_dependencies(cmocka-tests test_scene_blend)
add_test(test_scene_blend ${CMAKE_CURRENT_BINARY_DIR}/test_scene_blend)
