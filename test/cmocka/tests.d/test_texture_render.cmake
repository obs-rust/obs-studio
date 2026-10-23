# Characterization of the gs_texrender_* state machine before its port to
# obs-graphics. A fake device in the test records the calls; no GPU, no display.
add_executable(test_texture_render test_texture_render.c)
target_include_directories(test_texture_render PRIVATE ${CMOCKA_INCLUDE_DIR})
target_link_libraries(test_texture_render PRIVATE OBS::libobs ${CMOCKA_LIBRARIES})

add_dependencies(cmocka-tests test_texture_render)
add_test(test_texture_render ${CMAKE_CURRENT_BINARY_DIR}/test_texture_render)
